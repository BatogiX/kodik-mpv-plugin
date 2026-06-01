use anyhow::{Context, Result};
use kodik_parser::KodikApiResponse;
use mpv_client::Handle;

use crate::kodik;
use crate::{mpv_ext::MpvExt, state::PluginState};

pub fn on_load(state: &mut PluginState, mpv: &mut Handle, payload: &crate::events::Payload) -> Result<()> {
    let shiki_metadata = {
        let metadata = state.metadata();
        let crate::events::MetaData::Shiki(shiki_metadata) = metadata
            .get(payload.metadata_key())
            .context("must be inserted in `expand`")?
        else {
            anyhow::bail!("shiki payload expected")
        };
        anyhow::Ok(shiki_metadata)
    }?;

    if !state.kodik_videos().contains_key(payload.metadata_key()) {
        let videos = state
            .runtime()
            .block_on(KodikApiResponse::fetch_shiki(state.client(), shiki_metadata.id))?;
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

    let direct_link = kodik::resolve_indirect_link(state, format!("https:{indirect_link}").as_str())?;
    mpv.set_stream_open_filename(direct_link)?;

    Ok(())
}
