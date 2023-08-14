use crate::AppCtx;
use dropshot::{
    endpoint, ApiDescription, HttpError, HttpResponseOk, Query, RequestContext, TypedBody,
};
use futures::stream::StreamExt;
use futures_util::stream::FuturesUnordered;
use schemars::JsonSchema;
use serde::Deserialize;
use sonor::Playlist;
use sonor::{rupnp::Device, Speaker};
use std::time::Duration;

#[derive(Deserialize, JsonSchema)]
struct SonosArgs {
    rooms: Vec<String>,
    volume: Option<u16>,
    sleep_timer: Option<u16>,
}

async fn goodnight(speaker: &sonor::Speaker, sleep_timer: Option<u16>) -> Result<(), sonor::Error> {
    speaker.stop().await?;
    // fails if the queue is already clear or in an unexpected state, so it's safe to ignore for
    // now as we are about to replace it.
    let _ = speaker.clear_queue().await;
    // TODO: make this a paramater.  For now hardcode it to the "Sleep" playlist.
    speaker
        .queue_next("file:///jffs/settings/savedqueues.rsq#23", "")
        .await?;
    speaker.set_repeat_mode(sonor::RepeatMode::All).await?;
    speaker.set_shuffle(true).await?;
    if let Some(t) = sleep_timer.map(|v| v.clamp(0, 2 * 60 * 60)) {
        speaker.set_sleep_timer(t as u64).await?;
    }
    speaker.play().await
}

async fn find_playlist(
    speaker: &sonor::Speaker,
    playlist: &str,
) -> Result<Option<Playlist>, sonor::Error> {
    let playlists = speaker.playlists().await?;
    Ok(playlists
        .into_iter()
        .find(|p| p.title().eq_ignore_ascii_case(playlist)))
}

async fn queue_playlist(
    speaker: &sonor::Speaker,
    playlist: Playlist,
    shuffle: bool,
    repeat: bool,
) -> Result<(), sonor::Error> {
    let _ = speaker.stop().await;
    let _ = speaker.clear_queue().await;
    let repeat = repeat
        .then_some(sonor::RepeatMode::All)
        .unwrap_or(sonor::RepeatMode::None);

    speaker.queue_next(playlist.uri(), "").await?;
    speaker.set_repeat_mode(repeat).await?;
    speaker.set_shuffle(shuffle).await?;
    speaker.play().await
}

async fn group_rooms(
    rctx: &RequestContext<AppCtx>,
    rooms: &[String],
    volume: Option<u16>,
) -> Result<Option<Speaker>, sonor::Error> {
    // Make sure we have at least one room passed in.
    let first = match rooms.first() {
        Some(c) => c,
        None => return Ok(None),
    };

    if let Some(coordinator) = sonor::find(first, Duration::from_secs(3)).await? {
        let find = coordinator
            .zone_group_state()
            .await?
            .into_iter()
            .flat_map(|(_, v)| v)
            .filter(|i| rooms[1..].iter().any(|n| n.eq_ignore_ascii_case(i.name())))
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

        let speakers: Vec<Speaker> = find.into_iter().filter_map(Result::ok).flatten().collect();

        let default_volume = coordinator.volume().await?;
        let volume = volume.unwrap_or(default_volume);

        coordinator.leave().await?;
        coordinator.set_volume(volume).await?;
        for speaker in speakers {
            speaker.leave().await?;
            speaker.set_volume(volume).await?;
            if let Err(e) = speaker.join(first).await {
                warn!(
                    rctx.log,
                    "failed to join {} to group: {}",
                    speaker.name().await?,
                    e
                )
            }
        }

        info!(rctx.log, "joined rooms: {:?}", rooms);
        return Ok(Some(coordinator));
    };
    Ok(None)
}

#[endpoint {
    method = POST,
    path = "/sonos/sleep",
}]
async fn sleep(
    rctx: RequestContext<AppCtx>,
    body_param: TypedBody<SonosArgs>,
) -> Result<HttpResponseOk<()>, HttpError> {
    let app = rctx.context();
    let req = &rctx.request;
    let _ = app.require_auth(&req)?;
    let body = body_param.into_inner();

    if let Some(speaker) = group_rooms(&rctx, &body.rooms, body.volume)
        .await
        .map_err(|e| HttpError::for_internal_error(format!("failed sonos request: {}", e)))?
    {
        goodnight(&speaker, body.sleep_timer)
            .await
            .map_err(|e| HttpError::for_unavail(None, format!("{}", e)))?;
    } else {
        return Err(HttpError::for_bad_request(
            None,
            format!("verify sonos speakers: [{:?}]", &body.rooms),
        ));
    }

    info!(rctx.log, "sleep mode initiated for: {:?}", &body.rooms);
    Ok(HttpResponseOk(()))
}

#[derive(Deserialize, JsonSchema)]
struct PlaylistQueryArgs {
    shuffle: Option<bool>,
    repeat: Option<bool>,
    sleep_timer: Option<u16>,
}

#[derive(Deserialize, JsonSchema)]
struct PlaylistArgs {
    rooms: Vec<String>,
    playlist: String,
    volume: Option<u16>,
}

#[endpoint {
    method = POST,
    path = "/sonos/playlist",
}]
async fn play_playlist(
    rctx: RequestContext<AppCtx>,
    query: Query<PlaylistQueryArgs>,
    body_param: TypedBody<PlaylistArgs>,
) -> Result<HttpResponseOk<()>, HttpError> {
    let app = rctx.context();
    let req = &rctx.request;
    let _ = app.require_auth(&req)?;
    let body = body_param.into_inner();
    let query = query.into_inner();

    let shuffle = query.shuffle.unwrap_or(false);
    let repeat = query.repeat.unwrap_or(false);

    let coordinator = match group_rooms(&rctx, &body.rooms, body.volume)
        .await
        .map_err(|e| HttpError::for_internal_error(format!("failed sonos request: {}", e)))?
    {
        Some(c) => c,
        None => {
            return Err(HttpError::for_not_found(
                None,
                "No coordinator found".to_string(),
            ))
        }
    };

    let playlist = match find_playlist(&coordinator, &body.playlist)
        .await
        .map_err(|e| HttpError::for_internal_error(format!("failed sonos request: {}", e)))?
    {
        Some(p) => p,
        None => {
            return Err(HttpError::for_not_found(
                None,
                "No playlist found".to_string(),
            ))
        }
    };

    queue_playlist(&coordinator, playlist, shuffle, repeat)
        .await
        .map_err(|e| {
            // XXX Try and get more info about why this fails
            error!(&rctx.log, "sonos request failed: {:?}", e);
            HttpError::for_internal_error(format!("failed sonos request: {}", e))
        })?;

    if let Some(t) = query.sleep_timer.map(|v| v.clamp(0, 2 * 60 * 60)) {
        coordinator.set_sleep_timer(t as u64).await.map_err(|e| {
            error!(&rctx.log, "sonos request failed: {:?}", e);
            HttpError::for_internal_error(format!("failed sonos request: {}", e))
        })?;
    }

    Ok(HttpResponseOk(()))
}

#[endpoint {
    method = POST,
    path = "/sonos/group",
}]
async fn group(
    rctx: RequestContext<AppCtx>,
    body_param: TypedBody<SonosArgs>,
) -> Result<HttpResponseOk<()>, HttpError> {
    let app = rctx.context();
    let req = &rctx.request;
    let _ = app.require_auth(&req)?;
    let body = body_param.into_inner();

    group_rooms(&rctx, &body.rooms, body.volume)
        .await
        .map_err(|e| HttpError::for_internal_error(format!("failed sonos request: {}", e)))?;
    Ok(HttpResponseOk(()))
}

pub fn mount(api: &mut ApiDescription<AppCtx>) {
    api.register(sleep).expect("failed to mount sleep");
    api.register(group).expect("failed to mount group");
    api.register(play_playlist)
        .expect("failed to mount play_playlist");
}
