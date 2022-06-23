use anyhow::anyhow;
use dropshot::{
    ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpError, HttpServerStarter,
};
use hyper::{Body, Request, StatusCode};
use illumos_priv::{PrivOp, PrivPtype, PrivSet, Privilege};
use shark::SharkClient;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;

#[macro_use]
extern crate slog;

mod config;
mod shark_endpoint;
mod sonos_endpoint;

const X_API_KEY: &str = "X-API-Key";

// XXX we may wish to return some value or control things via ACL someday
pub struct Auth;

type AppCtx = Arc<App>;
pub struct App {
    shark: RwLock<SharkClient>,
    auth_tokens: Vec<String>,
}

impl App {
    fn require_auth(&self, req: &Request<Body>) -> Result<Auth, HttpError> {
        let token = match req.headers().get(X_API_KEY) {
            Some(h) => match h.to_str() {
                Ok(t) => Some(t),
                Err(_) => None,
            },
            None => None,
        };

        if let Some(t) = token {
            if self.auth_tokens.iter().any(|token| token == t.trim()) {
                return Ok(Auth);
            }
        }

        Err(HttpError::for_client_error(
            None,
            StatusCode::UNAUTHORIZED,
            "invalid x-api-key header".to_string(),
        ))
    }
}

fn drop_privs() -> anyhow::Result<()> {
    let mut pset = PrivSet::new_basic()?;
    pset.delset(Privilege::ProcFork)?;
    pset.delset(Privilege::ProcExec)?;
    pset.delset(Privilege::ProcInfo)?;
    pset.delset(Privilege::ProcSession)?;
    illumos_priv::setppriv(PrivOp::Set, PrivPtype::Permitted, &pset)?;
    illumos_priv::setppriv(PrivOp::Set, PrivPtype::Limit, &pset)?;
    Ok(())
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

    let host = config.host.unwrap_or_else(|| "127.0.0.1".parse().unwrap());
    let port = config.port.unwrap_or(8080);
    let sa = SocketAddr::new(host, port);

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
            bind_address: sa,
            request_body_max_bytes: 1024,
            tls: None,
        },
        api,
        appctx,
        &log,
    )
    .map_err(|error| anyhow!("failed to start server: {}", error))?;

    drop_privs().map_err(|e| anyhow!("Failed to drop privs: {}", e))?;

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
