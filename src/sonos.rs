use dropshot::{
    endpoint, ApiDescription, HttpError, HttpResponseOk, Path, RequestContext, TypedBody,
};
use futures::stream::StreamExt;
use futures_util::stream::FuturesUnordered;
use schemars::JsonSchema;
use serde::Deserialize;
use sonor::{rupnp::Device, Speaker};
use std::sync::Arc;
use std::time::Duration;

#[derive(Deserialize, JsonSchema)]
struct SonosSleepArgs {
    room: String,
}

#[derive(Deserialize, JsonSchema)]
struct SonosGroupArgs {
    rooms: Vec<String>,
    volume: Option<u16>,
}

async fn goodnight(speaker: &sonor::Speaker) -> Result<(), sonor::Error> {
    speaker.stop().await?;
    speaker.leave().await?;
    speaker.set_volume(5).await?;
    // fails if the queue is already clear or in an unexpected state, so it's safe to ignore for
    // now as we are about to replace it.
    let _ = speaker.clear_queue().await;
    // TODO: make this a paramater.  For now hardcode it to the "Sleep" playlist.
    speaker
        .queue_next("file:///jffs/settings/savedqueues.rsq#23", "")
        .await?;
    speaker.set_repeat_mode(sonor::RepeatMode::All).await?;
    speaker.set_shuffle(true).await?;
    speaker.set_sleep_timer(2 * 60 * 60).await?;
    speaker.play().await
}

async fn group_rooms(
    rctx: Arc<RequestContext>,
    rooms: &[String],
    volume: Option<u16>,
) -> Result<(), sonor::Error> {
    // Make sure we have at least one room passed in.
    let first = match rooms.first() {
        Some(c) => c,
        None => return Ok(()),
    };

    if let Some(coordinator) = sonor::find(first, Duration::from_secs(3)).await? {
        let find = coordinator
            .zone_group_state()
            .await?
            .into_iter()
            .flat_map(|(_, v)| v)
            .filter(|i| {
                if let Some(_) = rooms[1..]
                    .iter()
                    .position(|n| n.eq_ignore_ascii_case(i.name()))
                {
                    return true;
                }
                false
            })
            .map(|info| {
                let url = info.location().parse();
                async {
                    let device = Device::from_url(url?).await?;
                    Ok(Speaker::from_device(device))
                }
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<Result<Option<Speaker>, sonor::Error>>>()
            .await;

        let speakers: Vec<Speaker> = find
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|s| s)
            .collect();

        let default_volume = coordinator.volume().await?;
        let volume = volume.unwrap_or(default_volume);

        coordinator.leave().await?;
        coordinator.set_volume(volume).await?;
        for speaker in speakers {
            speaker.leave().await?;
            speaker.set_volume(volume).await?;
            let _ = speaker.join(first).await;
        }
    };

    info!(rctx.log, "joined rooms: {:?}", rooms);
    Ok(())
}

#[endpoint {
    method = PUT,
    path = "/sonos/sleep/{room}",
}]
async fn sleep(
    _rctx: Arc<RequestContext>,
    path: Path<SonosSleepArgs>,
) -> Result<HttpResponseOk<()>, HttpError> {
    let path_params = path.into_inner();
    let roomname = path_params.room;
    if let Some(speaker) = sonor::find(&roomname, Duration::from_secs(3))
        .await
        .map_err(|e| HttpError::for_internal_error(format!("failed sonos request: {}", e)))?
    {
        goodnight(&speaker)
            .await
            .map_err(|e| HttpError::for_unavail(None, format!("{}", e)))?;
    } else {
        return Err(HttpError::for_not_found(
            None,
            format!("sonos zone {} not found", &roomname),
        ));
    }

    Ok(HttpResponseOk(()))
}

#[endpoint {
    method = POST,
    path = "/sonos/group",
}]
async fn group(
    rctx: Arc<RequestContext>,
    body_param: TypedBody<SonosGroupArgs>,
) -> Result<HttpResponseOk<()>, HttpError> {
    let body = body_param.into_inner();
    let context = Arc::clone(&rctx);
    group_rooms(context, &body.rooms, body.volume)
        .await
        .map_err(|e| HttpError::for_internal_error(format!("failed sonos request: {}", e)))?;
    Ok(HttpResponseOk(()))
}

pub fn mount(api: &mut ApiDescription) {
    api.register(sleep).expect("failed to mount sleep");
    api.register(group).expect("failed to mount group");
}
