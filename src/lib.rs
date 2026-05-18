use std::ffi::c_int;

use mpv_client::{Event, mpv_handle};

mod config;
mod hooks;
mod logger;
mod models;
mod mpv_ext;
mod related;
mod shiki;
mod state;

use crate::mpv_ext::MpvResultExt;
use crate::state::PluginState;

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "C" fn mpv_open_cplugin(handle: *mut mpv_handle) -> c_int {
    log::set_max_level(log::LevelFilter::Error);

    let mut state = match PluginState::new(handle) {
        Ok(state) => state,
        Err(err) => {
            log::error!("failed to initialize plugin state: {err:?}");
            return 1;
        }
    };

    logger::init_logger(state.mpv_mut().name(), state.config().log_level());

    if let Err(err) = hooks::register(&mut state) {
        log::error!("failed to register hooks: {err:?}");
        return 1;
    }

    if let Err(err) = related::expand_by_related(&mut state) {
        log::error!("failed to expand by related: {err:?}");
        return 1;
    }

    loop {
        match state.mpv_mut().wait_event(-1.) {
            Event::Shutdown => return 0,
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
                    && let Err(err) = shiki::mark_as_watched(&mut state)
                {
                    log::error!("failed to mark as watched: {err:?}");
                }
            }
            _ => {}
        }
    }
}
