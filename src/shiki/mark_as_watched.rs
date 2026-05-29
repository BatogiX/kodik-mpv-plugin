use std::{fmt::Display, str::FromStr, time::Duration};

use crate::mpv_ext::MpvExt;
use crate::shiki::{COMPLETED_CHAR, REWATCHING_CHAR, WATCHING_CHAR};
use crate::{
    events::{MetaData, Payload},
    shiki::{ShikiApiUserRates, ShikiMetaData, UserRatesTargetType},
};
use anyhow::{Context, Result};
use kodik_shiki::{AnimeStatus, UserRate, UserRateStatus};
use kodik_utils::{PATCH as _, POST};
use mpv_client::Handle;
use reqwest::{Url, cookie::CookieStore};

use crate::state::PluginState;

pub fn mark_as_watched(state: &mut PluginState, mpv: &mut Handle, payload: Payload) -> Result<()> {
    let user_id = {
        let metadata = state
            .metadata()
            .get(payload.metadata_key())
            .context("must be inserted in `expand`")?;

        let MetaData::Shiki(shiki_metadata) = metadata else {
            anyhow::bail!("shiki metadata expected")
        };

        let url = Url::from_str(&format!("https://{}", shiki_metadata.host))?;

        let has_kawai_session = state
            .jar()
            .cookies(&url)
            .map(|cookies| cookies.to_str().map(|s| s.contains("_kawai_session")))
            .transpose()?
            .unwrap_or(false);

        if state.config().cookies().is_none() || !has_kawai_session {
            anyhow::bail!("there is no cookies for `{url}`");
        }

        let Some(user_id) = shiki_metadata.user_id else {
            anyhow::bail!("there is no `user_id` in payload")
        };

        anyhow::Ok(user_id)
    }?;

    let current_pos: i64 = mpv.get_playlist_pos()?;
    let handle = state.runtime().handle().clone();

    handle.block_on(async {
        let result = async {
            let Some(metadata) = state.metadata().get(payload.metadata_key()) else {
                anyhow::bail!("must be inserted in `expand`");
            };

            let MetaData::Shiki(shiki_metadata) = metadata else {
                anyhow::bail!("shiki payload expected");
            };

            let is_last_episode = payload.episode() == shiki_metadata.episodes;
            let (osd_text, user_rate) = if let Some(user_rate) = shiki_metadata.user_rate.as_ref() {
                let (user_rate_rewatches, user_rate_status, completed_rewatch) = if is_last_episode {
                    let completed_rewatch =
                        user_rate.status == UserRateStatus::Rewatching || user_rate.status == UserRateStatus::Completed;

                    let rewatches = if completed_rewatch {
                        user_rate.rewatches + 1
                    } else {
                        user_rate.rewatches
                    };

                    (rewatches, UserRateStatus::Completed, completed_rewatch)
                } else if user_rate.status == UserRateStatus::Completed
                    || user_rate.status == UserRateStatus::Rewatching
                {
                    (user_rate.rewatches, UserRateStatus::Rewatching, false)
                } else {
                    (user_rate.rewatches, UserRateStatus::Watching, false)
                };

                let shiki_api_user_rates = ShikiApiUserRates::new(
                    payload.episode(),
                    user_rate_rewatches,
                    user_rate_status,
                    shiki_metadata.id,
                    UserRatesTargetType::Anime,
                    user_id,
                );

                let user_rate: UserRate = state
                    .client()
                    .patch_json_as_json(
                        &format!("https://{}/api/v2/user_rates/{}", shiki_metadata.host, user_rate.id),
                        &shiki_api_user_rates,
                    )
                    .await?;

                (
                    mark_as_watched_osd_text(
                        payload.episode(),
                        shiki_metadata.episodes,
                        user_rate_status,
                        user_rate_rewatches,
                        completed_rewatch,
                    ),
                    user_rate,
                )
            } else {
                let user_rate_status = if is_last_episode {
                    UserRateStatus::Completed
                } else {
                    UserRateStatus::Watching
                };

                let shiki_api_user_rates = ShikiApiUserRates::new(
                    payload.episode(),
                    0,
                    user_rate_status,
                    shiki_metadata.id,
                    UserRatesTargetType::Anime,
                    user_id,
                );

                let user_rate: UserRate = state
                    .client()
                    .post_json_as_json(
                        &format!("https://{}/api/v2/user_rates", shiki_metadata.host),
                        &shiki_api_user_rates,
                    )
                    .await?;

                (
                    mark_as_watched_osd_text(payload.episode(), shiki_metadata.episodes, user_rate_status, 0, false),
                    user_rate,
                )
            };

            let Some(metadata) = state.metadata_mut().get_mut(payload.metadata_key()) else {
                anyhow::bail!("must be inserted in `expand`");
            };

            let MetaData::Shiki(shiki_metadata) = metadata else {
                anyhow::bail!("shiki payload expected");
            };

            shiki_metadata.user_rate = Some(user_rate);
            update_playlist_watched_titles(mpv, shiki_metadata, current_pos, payload.metadata_key())?;
            let _ = mpv_client::osd!(mpv, Duration::from_secs(8), "{osd_text}");

            anyhow::Ok(())
        }
        .await;

        if let Err(err) = result {
            log::error!("failed to mark episode as watched: {err:?}");
        }
    });

    let () = mpv.playlist_play_index(&(current_pos + 1).to_string())?;

    Ok(())
}

fn mark_as_watched_osd_text(
    episode: impl Display,
    episodes: impl Display,
    status: UserRateStatus,
    rewatches: impl Display,
    completed_rewatch: bool,
) -> String {
    match status {
        UserRateStatus::Completed if completed_rewatch => {
            format!("{COMPLETED_CHAR} Rewatch completed: {episode}/{episodes} — rewatch #{rewatches}")
        }
        UserRateStatus::Completed => format!("{COMPLETED_CHAR} Marked as completed: {episode}/{episodes}"),
        UserRateStatus::Rewatching => {
            format!("{REWATCHING_CHAR} Marked as watched: {episode}/{episodes} — rewatch #{rewatches}")
        }
        UserRateStatus::Watching => format!("{WATCHING_CHAR} Marked as watched: {episode}/{episodes}"),
        _ => String::new(),
    }
}

fn update_playlist_watched_titles(
    mpv: &mut Handle,
    shiki_metadata: &ShikiMetaData,
    current_pos: i64,
    metadata_key: &str,
) -> Result<()> {
    let user_rate = &shiki_metadata.user_rate.context("user rate must exist after request")?;

    let status_marker = if user_rate.status == UserRateStatus::Watching {
        WATCHING_CHAR
    } else {
        REWATCHING_CHAR
    };

    let episodes = if shiki_metadata.status == AnimeStatus::Ongoing {
        shiki_metadata.episodes_aired
    } else {
        shiki_metadata.episodes
    };

    let mut update_title = |index: i64, episode, status_marker| -> Result<()> {
        let media_title = mpv.get_playlist_filename_by_index(index)?;

        if media_title.ends_with(status_marker) {
            return Ok(());
        }

        let media_title = {
            let last_char = media_title.chars().next_back().unwrap();
            let base = &media_title[..media_title.len() - last_char.len_utf8()];

            if last_char.is_ascii_digit() {
                format!("{base}{last_char} {status_marker}")
            } else {
                format!("{base}{status_marker}")
            }
        };

        let payload = Payload::new(metadata_key.to_owned(), episode);
        mpv.loadfile_insert_at(&media_title, &index.to_string(), &payload.encode()?)?;
        mpv.playlist_remove(index + 1)?;

        Ok(())
    };

    for (index, episode) in (0..=current_pos).rev().zip((1..=user_rate.episodes).rev()) {
        update_title(index, episode, COMPLETED_CHAR)?;
    }

    for (index, episode) in (current_pos + 1..=i64::MAX).zip(user_rate.episodes + 1..=episodes) {
        update_title(index, episode, status_marker)?;
    }

    Ok(())
}
