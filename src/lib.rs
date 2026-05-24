use std::ffi::c_int;

use log::LevelFilter;
use mpv_client::{Event, mpv_handle};

mod cache;
mod config;
mod hooks;
mod logger;
mod mpv_ext;
mod shiki;
mod state;

use crate::cache::Cache;
use crate::mpv_ext::MpvResultExt;
use crate::state::PluginState;

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "C" fn mpv_open_cplugin(handle: *mut mpv_handle) -> c_int {
    log::set_max_level(LevelFilter::Error);

    let mut state = match PluginState::new(handle) {
        Ok(state) => state,
        Err(err) => {
            log::error!("failed to initialize plugin state: {err:?}");
            return 1;
        }
    };

    logger::init_logger(state.mpv_mut().name(), state.config().log_level());

    let mut cache = match Cache::load(state.mpv_mut()) {
        Ok(cache) => cache,
        Err(err) => {
            log::error!("failed to load cache: {err:?}");
            return 1;
        }
    };

    if let Err(err) = hooks::register(&mut state) {
        log::error!("failed to register hooks: {err:?}");
        return 1;
    }

    loop {
        match state.mpv_mut().wait_event(-1.) {
            Event::Shutdown => break,
            Event::Hook(_, hook) => {
                if let Err(err) = hooks::handle_hook(&mut state, &hook) {
                    log::error!("hook failed: {err:?}");
                }

                if let Err(err) = state.mpv_mut().hook_continue(hook.id()) {
                    log::error!("failed to continue hook {}: {:?}", hook.id(), err);
                }
            }
            Event::ClientMessage(data) => {
                if let ["key-binding", "watched", "u--", ..] = data.args().as_slice()
                    && let Err(err) = hooks::mark_as_watched(&mut state)
                {
                    log::error!("failed to mark as watched: {err:?}");
                }
            }
            _ => {}
        }
    }

    if let Err(err) = cache.update_and_save() {
        log::error!("failed to update and save cache: {err}");
        return 1;
    }

    0
}
