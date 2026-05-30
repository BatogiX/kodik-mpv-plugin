use crate::{
    config::Quality,
    events::{MetaData, Payload},
    mpv_ext::MpvExt,
};
use anyhow::{Context as _, Result};
use kodik_utils::GET as _;
use mpv_client::Handle;

use crate::state::PluginState;

pub fn on_load(state: &mut PluginState, mpv: &mut Handle, payload: &Payload) -> Result<()> {
    let shiki_metadata = {
        let metadata = state.metadata();

        let MetaData::Shiki(shiki_metadata) = metadata
            .get(payload.metadata_key())
            .context("must be inserted in `expand`")?
        else {
            anyhow::bail!("shiki payload expected")
        };

        anyhow::Ok(shiki_metadata)
    }?;

    // TODO: Merge in one if let
    if !state.kodik_videos().contains_key(payload.metadata_key()) {
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

    let result = &kodik_videos.find_result(state.config().translation_title(), state.config().translation_type())?;

    let indirect_link = if let Some(seasons) = result.seasons.as_ref() {
        let mut episodes_accum = 0;
        let mut found = None;

        for (_, season) in seasons.iter().filter(|(number, _)| **number > 0) {
            let Some((&last_episode, _)) = season.episodes.last_key_value() else {
                anyhow::bail!("season must have episodes");
            };

            if episodes_accum + last_episode < payload.episode() {
                episodes_accum += last_episode;
                continue;
            }

            found = season.episodes.get(&(payload.episode() - episodes_accum));

            break;
        }

        found
    } else {
        Some(&result.link)
    };

    let Some(indirect_link) = indirect_link else {
        mpv.playlist_next_weak()?;
        anyhow::bail!("episode not found");
    };

    let mut links = state.runtime().block_on(async {
        let links = kodik_parser::parse(state.client(), format!("https:{indirect_link}").as_str()).await?;
        Ok::<[String; 3], anyhow::Error>([links.p720, links.p480, links.p360])
    })?;

    match state.config().quality() {
        Quality::P720 => {}
        Quality::P480 => links.swap(1, 0),
        Quality::P360 => links.swap(2, 0),
    }

    let mut direct_link = None;
    for link in links {
        let text = state
            .runtime()
            .block_on(async { state.client().fetch_as_text(&link).await })?;

        if text.is_empty() {
            continue;
        }

        direct_link = Some(link);
        break;
    }

    let Some(direct_link) = direct_link else {
        anyhow::bail!("invalid links")
    };

    mpv.set_stream_open_filename(direct_link)?;

    Ok(())
}
