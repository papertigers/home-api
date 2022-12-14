use crate::AppCtx;
use dropshot::{
    endpoint, ApiDescription, HttpError, HttpResponseAccepted, HttpResponseOk, Path, RequestContext,
};
use schemars::JsonSchema;
use serde::Deserialize;
use shark::SharkDevice;
use std::sync::Arc;

#[derive(Deserialize, JsonSchema)]
struct ActionPathParam {
    dsn: String,
}

#[endpoint {
    method = GET,
    path = "/shark/devices",
}]
async fn get_devices(
    rctx: Arc<RequestContext<AppCtx>>,
) -> Result<HttpResponseOk<Vec<SharkDevice>>, HttpError> {
    let app = rctx.context();
    let req = rctx.request.lock().await;
    let _ = app.require_auth(&req)?;

    let shark = app.shark.read().await;
    match shark.get_devices().await {
        Ok(devices) => Ok(HttpResponseOk(devices)),
        Err(e) => Err(HttpError::for_internal_error(format!(
            "shark api error: {}",
            e
        ))),
    }
}

#[endpoint {
    method = PUT,
    path = "/shark/devices/{dsn}/start",
}]
async fn start(
    rctx: Arc<RequestContext<AppCtx>>,
    path_params: Path<ActionPathParam>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let app = rctx.context();
    let req = rctx.request.lock().await;
    let _ = app.require_auth(&req)?;
    let shark = app.shark.read().await;
    let dsn = path_params.into_inner().dsn;

    match shark
        .set_device_operating_mode(&dsn, shark::OperatingMode::Start)
        .await
    {
        Ok(_) => Ok(HttpResponseAccepted(())),
        Err(e) => Err(HttpError::for_internal_error(e.to_string())),
    }
}

#[endpoint {
    method = PUT,
    path = "/shark/devices/{dsn}/stop",
}]
async fn stop(
    rctx: Arc<RequestContext<AppCtx>>,
    path_params: Path<ActionPathParam>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let app = rctx.context();
    let req = rctx.request.lock().await;
    let _ = app.require_auth(&req)?;
    let shark = app.shark.read().await;
    let dsn = path_params.into_inner().dsn;

    match shark
        .set_device_operating_mode(&dsn, shark::OperatingMode::Stop)
        .await
    {
        Ok(_) => Ok(HttpResponseAccepted(())),
        Err(e) => Err(HttpError::for_internal_error(e.to_string())),
    }
}

#[endpoint {
    method = PUT,
    path = "/shark/devices/{dsn}/return",
}]
async fn r#return(
    rctx: Arc<RequestContext<AppCtx>>,
    path_params: Path<ActionPathParam>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let app = rctx.context();
    let req = rctx.request.lock().await;
    let _ = app.require_auth(&req)?;
    let shark = app.shark.read().await;
    let dsn = path_params.into_inner().dsn;

    match shark
        .set_device_operating_mode(&dsn, shark::OperatingMode::Return)
        .await
    {
        Ok(_) => Ok(HttpResponseAccepted(())),
        Err(e) => Err(HttpError::for_internal_error(e.to_string())),
    }
}

// #[endpoint {
//     method = PUT,
//     path = "/shark/devices/{dsn}/example",
// }]
// async fn example(
//     rctx: Arc<RequestContext<Arc<String>>>,
//     _path_params: Path<ActionPathParam>,
// ) -> Result<HttpResponseAccepted<()>, HttpError> {
//     let app = rctx.context();
//     todo!()
// }

pub fn mount(api: &mut ApiDescription<AppCtx>) {
    api.register(get_devices)
        .expect("failed to register get_devices");
    api.register(start).expect("failed to register start");
    api.register(stop).expect("failed to register stop");
    api.register(r#return).expect("failed to register return");
    // api.register(example).expect("failed to register return");
}
