use std::{str::FromStr, time::Duration};

use crate::mpv_ext::MpvExt;
use crate::shiki::{COMPLETED_CHAR, REWATCHING_CHAR, WATCHING_CHAR};
use crate::{
    events::{MetaData, Payload},
    shiki::ShikiMetaData,
};
use anyhow::{Context, Result};
use kodik_shiki::{AnimeStatus, UserRate, UserRateStatus, UserRates, UserRatesTargetType};
use mpv_client::Handle;
use reqwest::{Url, cookie::CookieStore};

use crate::state::PluginState;

pub fn mark_as_watched(state: &mut PluginState, mp: &Handle, payload: &Payload) -> Result<()> {
    let (metadata_key, episode) = (payload.metadata_key(), payload.episode());

    let user_id = {
        let metadata = state
            .metadata()
            .get(metadata_key)
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

        anyhow::Ok(shiki_metadata.user_id)
    }?;

    let Some(user_id) = user_id else {
        anyhow::bail!("there is no `user_id` in payload")
    };

    let current_pos = mp.get_playlist_pos()?;
    let last_pos = mp.get_playlist_count()? - 1;
    let next_pos = current_pos + 1;

    let user_rate = state
        .runtime()
        .block_on(update_user_rate_and_osd(state, metadata_key, episode, user_id))?;

    let Some(metadata) = state.metadata_mut().get_mut(metadata_key) else {
        anyhow::bail!("must be inserted in `expand`")
    };
    let MetaData::Shiki(shiki_metadata) = metadata else {
        anyhow::bail!("shiki payload expected")
    };

    shiki_metadata.user_rate = Some(user_rate);
    update_playlist_watched_titles(mp, shiki_metadata, current_pos, metadata_key)?;
    let osd_text = mark_as_watched_osd_text(&user_rate, shiki_metadata);
    let _ = mpv_client::osd!(mp, Duration::from_secs(8), "{osd_text}");

    if current_pos != last_pos {
        mp.playlist_play_index(next_pos.to_string())?;
    }

    Ok(())
}

async fn update_user_rate_and_osd(
    state: &PluginState,
    metadata_key: &str,
    episode: usize,
    user_id: usize,
) -> Result<UserRate> {
    let shiki_metadata = state
        .metadata()
        .get(metadata_key)
        .context("must be inserted in `expand`")?;

    let MetaData::Shiki(shiki_metadata) = shiki_metadata else {
        anyhow::bail!("shiki payload expected")
    };

    let is_last_episode = episode == shiki_metadata.episodes;

    let user_rate = if let Some(user_rate) = shiki_metadata.user_rate.as_ref() {
        let (rewatches, status) = if is_last_episode
            && (user_rate.status == UserRateStatus::Rewatching || user_rate.status == UserRateStatus::Completed)
        {
            (user_rate.rewatches + 1, UserRateStatus::Completed)
        } else if user_rate.status == UserRateStatus::Completed || user_rate.status == UserRateStatus::Rewatching {
            (user_rate.rewatches, UserRateStatus::Rewatching)
        } else {
            (user_rate.rewatches, UserRateStatus::Watching)
        };

        let user_rates = UserRates::new(
            episode,
            rewatches,
            status,
            shiki_metadata.id,
            UserRatesTargetType::Anime,
            user_id,
        );

        user_rates
            .patch(state.client(), &shiki_metadata.host, user_rate.id)
            .await?
    } else {
        let status = if is_last_episode {
            UserRateStatus::Completed
        } else {
            UserRateStatus::Watching
        };

        let user_rates = UserRates::new(
            episode,
            0,
            status,
            shiki_metadata.id,
            UserRatesTargetType::Anime,
            user_id,
        );

        user_rates.post(state.client(), &shiki_metadata.host).await?
    };

    anyhow::Ok(user_rate)
}

fn mark_as_watched_osd_text(user_rate: &UserRate, anime: &ShikiMetaData) -> String {
    let (status, episode, rewatches) = (user_rate.status, user_rate.episodes, user_rate.rewatches);

    let episodes = if anime.status == AnimeStatus::Ongoing {
        anime.episodes_aired
    } else {
        anime.episodes
    };

    match status {
        UserRateStatus::Completed if rewatches > 0 => {
            format!("{COMPLETED_CHAR} Rewatch completed: {episode}/{episodes} — rewatch #{rewatches}")
        }
        UserRateStatus::Completed => format!("{COMPLETED_CHAR} Marked as completed: {episode}/{episodes}"),
        UserRateStatus::Rewatching => {
            format!("{REWATCHING_CHAR} Marked as rewatched: {episode}/{episodes} — rewatch #{rewatches}")
        }
        UserRateStatus::Watching => format!("{WATCHING_CHAR} Marked as watched: {episode}/{episodes}"),
        _ => String::new(),
    }
}

fn update_playlist_watched_titles(
    mp: &Handle,
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

    let update_title = |index: i64, episode, status_marker| -> Result<()> {
        let media_title = mp.get_playlist_filename_by_index(index)?;

        if media_title.ends_with(status_marker) {
            return Ok(());
        }

        let media_title = {
            let last_char = media_title
                .chars()
                .next_back()
                .context("media_title must not be empty")?;
            let base = &media_title[..media_title.len() - last_char.len_utf8()];

            if last_char.is_ascii_digit() {
                format!("{base}{last_char} {status_marker}")
            } else {
                format!("{base}{status_marker}")
            }
        };

        let payload = Payload::new(metadata_key.to_owned(), episode);
        mp.loadfile_insert_at(&media_title, &index.to_string(), &payload.encode()?)?;
        mp.playlist_remove(index + 1)?;

        Ok(())
    };

    for (index, episode) in (0..=current_pos).rev().zip((1..=user_rate.episodes).rev()) {
        update_title(index, episode, COMPLETED_CHAR)?;
    }

    for (index, episode) in (current_pos + 1..i64::MAX).zip(user_rate.episodes + 1..=episodes) {
        update_title(index, episode, status_marker)?;
    }

    Ok(())
}
