use crate::{config::Quality, mpv_ext::MpvExt as _, state::PluginState};
use anyhow::Result;
use kodik_utils::ClientExt as _;
use mpv_client::Handle;

pub fn on_load(state: &PluginState, mpv: &mut Handle, indirect_link: &str) -> Result<()> {
    let mut links = state.runtime().block_on(async {
        let links = kodik_parser::parse(state.client(), indirect_link).await?;
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
