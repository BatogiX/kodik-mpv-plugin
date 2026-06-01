use anyhow::Result;
use kodik_parser::Links;
use kodik_utils::ClientExt as _;
use mpv_client::Handle;

use crate::{config::Quality, mpv_ext::MpvExt, state::PluginState};

pub fn on_load(state: &PluginState, mpv: &mut Handle, indirect_link: &str) -> Result<()> {
    let direct_link = resolve_indirect_link(state, indirect_link)?;
    mpv.set_stream_open_filename(direct_link)?;
    Ok(())
}

pub fn resolve_indirect_link(state: &PluginState, indirect_link: &str) -> Result<String> {
    let links = state
        .runtime()
        .block_on(async { Links::fetch(state.client(), indirect_link).await })?;

    let mut links = [links.p720, links.p480, links.p360];

    match state.config().quality() {
        Quality::P720 => {}
        Quality::P480 => links.swap(1, 0),
        Quality::P360 => links.swap(2, 0),
    }

    for link in links {
        let text = state
            .runtime()
            .block_on(async { state.client().fetch_as_text(&link).await })?;

        if !text.is_empty() {
            return Ok(link);
        }
    }

    anyhow::bail!("invalid links");
}
