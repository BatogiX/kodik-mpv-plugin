use anyhow::Result;
use mpv_client::Hook;

use crate::MpvResultExt as _;
use crate::state::PluginState;

const HOOK_ON_LOAD: u64 = 1;

pub fn register(state: &mut PluginState) -> Result<()> {
    state
        .mpv_mut()
        .hook_add(HOOK_ON_LOAD, "on_load", 50)
        .mpv_context("failed to add on_load hook")?;

    Ok(())
}

pub fn handle_hook(_state: &mut PluginState, hook: &Hook) -> Result<()> {
    match hook.name() {
        // "on_load" => related::expand_by_related(state),
        _ => Ok(()),
    }
}
