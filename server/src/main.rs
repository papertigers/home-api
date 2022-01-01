use anyhow::anyhow;
use dropshot::{
    ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpError, HttpServerStarter,
};
use hyper::{Body, Request, StatusCode};
use illumos_priv::{PrivOp, PrivPtype, PrivSet, Privilege};
use shark::SharkClient;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;

#[macro_use]
extern crate slog;

mod config;
mod shark_endpoint;
mod sonos_endpoint;

pub struct Auth;

type AppCtx = Arc<App>;
pub struct App {
    shark: RwLock<SharkClient>,
    auth_tokens: Vec<String>,
}

impl App {
    async fn require_auth(&self, req: &Request<Body>) -> Result<Auth, HttpError> {
        // TODO validate against user tokens
        Err(HttpError::for_client_error(
            None,
            StatusCode::UNAUTHORIZED,
            "invalid Authorization header".to_string(),
        ))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = &args[0].clone();
    let brief = format!("Usage: {} [options] -c CONFIG", program);

    let mut opts = getopts::Options::new();
    opts.reqopt("c", "", "config file", "CONFIG");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => return Err(anyhow!("{}\n{}", e, opts.usage(&brief))),
    };

    let config = config::Config::from_file(matches.opt_str("c").unwrap())
        .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;

    let shark = SharkClient::builder(&config.shark.user, &config.shark.password)
        .build()
        .await
        .map_err(|e| anyhow!("failed to create shark client: {}", e))?;

    let app = Arc::new(App {
        shark: RwLock::new(shark),
        auth_tokens: config.user_auth,
    });
    let appctx = Arc::clone(&app);

    let log = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    }
    .to_logger("home-api")
    .map_err(|e| anyhow!("{}", e))?;

    let mut api = ApiDescription::new();
    sonos_endpoint::mount(&mut api);
    shark_endpoint::mount(&mut api);

    let server = HttpServerStarter::new(
        &ConfigDropshot {
            bind_address: "0.0.0.0:80".parse().unwrap(),
            request_body_max_bytes: 1024,
        },
        api,
        appctx,
        &log,
    )
    .map_err(|error| anyhow!("failed to start server: {}", error))?;

    let mut pset = PrivSet::new_basic().unwrap();
    pset.delset(Privilege::ProcFork).unwrap();
    pset.delset(Privilege::ProcExec).unwrap();
    pset.delset(Privilege::ProcInfo).unwrap();
    pset.delset(Privilege::ProcSession).unwrap();
    illumos_priv::setppriv(PrivOp::Set, PrivPtype::Permitted, &pset).unwrap();
    illumos_priv::setppriv(PrivOp::Set, PrivPtype::Limit, &pset).unwrap();

    tokio::task::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(60 * 60 * 12));
        interval.tick().await;

        loop {
            interval.tick().await;
            let mut shark = app.shark.write().await;
            match shark.refresh_token().await {
                Ok(_) => info!(&log, "refreshed shark access_token"),
                Err(e) => error!(&log, "error refreshing shark token: {}", e),
            }
        }
    });

    let server_task = server.start();
    server_task.await.map_err(|e| anyhow!("{}", e))
}