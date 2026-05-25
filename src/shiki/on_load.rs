use crate::{
    hooks::{MetaData, Payload},
    mpv_ext::MpvExt,
    shiki::ShikiMetaData,
};
use anyhow::{Context as _, Result};
use kodik_utils::GET as _;
use lazy_regex::{Lazy, Regex};

use crate::{config::Quality, state::PluginState};

const EXTRACT_HEIGHT_PATTERN: &Lazy<Regex> = lazy_regex::regex!(r"height<=\??(\d+)");

pub fn on_load(state: &mut PluginState, payload: &Payload) -> Result<()> {
    let shiki_metadata: ShikiMetaData = {
        let metadata = state.metadata_mut();

        let MetaData::Shiki(shiki_metadata) = metadata
            .get(payload.metadata_key())
            .expect("must be inserted in `expand`")
        else {
            anyhow::bail!("shiki payload expected")
        };

        anyhow::Ok(shiki_metadata.to_owned())
    }?;

    if !state.kodik_videos_mut().contains_key(payload.metadata_key()) {
        let videos = state
            .runtime()
            .block_on(kodik_shiki::fetch_kodik_videos(state.client(), shiki_metadata.id))?;

        state
            .kodik_videos_mut()
            .insert(payload.metadata_key().to_owned(), videos);
    }

    let kodik_videos = state
        .kodik_videos()
        .get(payload.metadata_key())
        .context("kodik videos should exist after insert")?;

    let result = match kodik_videos.find_result(state.config().translation_title(), state.config().translation_type()) {
        Ok(result) => result,
        Err(err) => {
            state.mpv_mut().playlist_next_weak()?;
            return Err(err.into());
        }
    };

    let Some(episode) = result.seasons.as_ref().map_or(Some(&result.link), |seasons| {
        seasons
            .iter()
            .last()
            .and_then(|(_, season)| season.episodes.get(&payload.episode()))
    }) else {
        state.mpv_mut().playlist_next_weak()?;
        anyhow::bail!("episode not found");
    };

    let links = state.runtime().block_on(async {
        let links = kodik_parser::parse(state.client(), format!("https:{episode}").as_str()).await?;
        Ok::<[String; 3], anyhow::Error>([links.p720, links.p480, links.p360])
    });

    let quality = EXTRACT_HEIGHT_PATTERN
        .captures(&state.mpv_mut().get_ytdl_format()?)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .map_or_else(
            || state.config().quality(),
            |height| match height {
                h if h >= 720 => Quality::P720,
                480 => Quality::P480,
                h if h <= 360 => Quality::P360,
                _ => state.config().quality(),
            },
        );

    if let Ok(mut links) = links {
        match quality {
            Quality::P720 => {}
            Quality::P480 => links.swap(1, 0),
            Quality::P360 => links.swap(2, 0),
        }

        for link in links {
            let text = state
                .runtime()
                .block_on(async { state.client().fetch_as_text(&link).await })?;

            if text.is_empty() {
                continue;
            }

            state.mpv_mut().set_stream_open_filename(link)?;

            break;
        }
    }

    Ok(())
}
