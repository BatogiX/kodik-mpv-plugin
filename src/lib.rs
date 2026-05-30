use std::ffi::c_int;

use log::LevelFilter;
use mpv_client::{Event, Handle, mpv_handle};

mod cache;
mod config;
mod events;
mod logger;
mod mpv_ext;
mod shiki;
mod state;

use crate::cache::Cache;
use crate::state::PluginState;

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "C" fn mpv_open_cplugin(handle: *mut mpv_handle) -> c_int {
    let mpv = Handle::from_ptr(handle);
    log::set_max_level(LevelFilter::Error);

    let mut state = match PluginState::new(mpv) {
        Ok(state) => state,
        Err(err) => {
            log::error!("failed to initialize plugin state: {err:?}");
            return 1;
        }
    };

    logger::init_logger(mpv.name(), state.config().log_level());

    let mut cache = match Cache::load(mpv) {
        Ok(cache) => cache,
        Err(err) => {
            log::error!("failed to load cache: {err:?}");
            return 1;
        }
    };

    if let Err(err) = events::register(mpv) {
        log::error!("failed to register hooks: {err:?}");
        return 1;
    }

    loop {
        match mpv.wait_event(-1.) {
            Event::Shutdown => break,
            Event::Hook(reply, hook) => {
                if let Err(err) = events::handle_event(&mut state, mpv, reply) {
                    log::error!("hook `{}` failed: {err:?}", hook.name());
                }

                if let Err(err) = mpv.hook_continue(hook.id()) {
                    log::error!("failed to continue hook `{}`: {:?}", hook.name(), err);
                }
            }
            Event::ClientMessage(data) => {
                if let ["key-binding", "watched", "d--" | "dm-", ..] = data.args().as_slice()
                    && let Err(err) = events::mark_as_watched(&mut state, mpv)
                {
                    log::error!("failed to mark as watched: {err:?}");
                }
            }
            Event::PropertyChange(reply, property) => {
                if let Err(err) = events::handle_event(&mut state, mpv, reply) {
                    log::error!("observe `{}` failed: {err:?}", property.name());
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
