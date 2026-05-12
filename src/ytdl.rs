use anyhow::{Result, anyhow};

use crate::state::PluginState;

const YTDL_EXCLUDE: &str = "ytdl_hook-exclude=^shikimori%.[^/]+/|^kodikplayer%.[^/]+/";

pub fn configure_ytdl_excludes(state: &mut PluginState) -> Result<()> {
    state
        .mpv_mut()
        .command(["change-list", "script-opts", "append", YTDL_EXCLUDE])
        .map_err(|err| anyhow!("failed to append ytdl_hook-exclude: {err:?}"))?;

    Ok(())
}
