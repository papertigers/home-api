use dropshot::ApiDescription;
use dropshot::ConfigDropshot;
use dropshot::ConfigLogging;
use dropshot::ConfigLoggingLevel;
use dropshot::HttpServer;
use std::sync::Arc;

#[macro_use]
extern crate slog;

mod sonos;

#[tokio::main]
async fn main() -> Result<(), String> {
    let log = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    }
    .to_logger("home-api")
    .map_err(|e| e.to_string())?;

    let mut api = ApiDescription::new();
    sonos::mount(&mut api);

    let mut server = HttpServer::new(
        &ConfigDropshot {
            bind_address: "0.0.0.0:80".parse().unwrap(),
            request_body_max_bytes: 1024,
        },
        api,
        Arc::new(()),
        &log,
    )
    .map_err(|error| format!("failed to start server: {}", error))?;

    let server_task = server.run();
    server.wait_for_shutdown(server_task).await
}
