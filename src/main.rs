use dropshot::ApiDescription;
use dropshot::ConfigDropshot;
use dropshot::ConfigLogging;
use dropshot::ConfigLoggingLevel;
use dropshot::HttpServerStarter;
use illumos_priv::{PrivOp, PrivPtype, PrivSet, Privilege};
use std::sync::Arc;

#[macro_use]
extern crate slog;

mod sonos;

pub struct App;
type AppCtx = Arc<App>;

#[tokio::main]
async fn main() -> Result<(), String> {
    let log = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    }
    .to_logger("home-api")
    .map_err(|e| e.to_string())?;

    let mut api = ApiDescription::new();
    sonos::mount(&mut api);

    let server = HttpServerStarter::new(
        &ConfigDropshot {
            bind_address: "0.0.0.0:80".parse().unwrap(),
            request_body_max_bytes: 1024,
        },
        api,
        Arc::new(App {}),
        &log,
    )
    .map_err(|error| format!("failed to start server: {}", error))?;

    // XXX Drop some privs?
    let mut pset = PrivSet::new_basic().unwrap();
    pset.delset(Privilege::ProcFork).unwrap();
    pset.delset(Privilege::ProcExec).unwrap();
    illumos_priv::setppriv(PrivOp::Set, PrivPtype::Permitted, &pset).unwrap();

    let server_task = server.start();
    server_task.await
}
