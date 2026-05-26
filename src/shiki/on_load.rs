use crate::{
    hooks::{LAZY_PLACEHOLDER_WEBM_B64, MetaData, Payload},
    mpv_ext::MpvExt,
};
use anyhow::{Context as _, Result};
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

    let chosen_title = &kodik_videos
        .find_result(state.config().translation_title(), state.config().translation_type())?
        .translation
        .title;

    mpv.video_add(LAZY_PLACEHOLDER_WEBM_B64, "select", chosen_title)?;
    mpv.set_stream_open_filename(LAZY_PLACEHOLDER_WEBM_B64)?;

    Ok(())
}
