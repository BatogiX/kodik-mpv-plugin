use mpv_client::{Event, EventQueueToken, Handle};

mod cache;
mod config;
mod events;
mod kodik;
mod mpv_ext;
mod shiki;
mod state;

use crate::cache::Cache;
use crate::state::PluginState;

#[mpv_client::main]
fn main(mp: &Handle, mut event_token: EventQueueToken) -> i32 {
    let mut state = match PluginState::new(mp) {
        Ok(state) => state,
        Err(err) => {
            log::error!("failed to initialize plugin state: {err:?}");
            return 1;
        }
    };

    let mut cache = match Cache::load(mp) {
        Ok(cache) => cache,
        Err(err) => {
            log::error!("failed to load cache: {err:?}");
            return 1;
        }
    };

    if let Err(err) = events::register(mp) {
        log::error!("failed to register hooks: {err:?}");
        return 1;
    }

    loop {
        match mp.wait_event(&mut event_token, -1.) {
            Event::Shutdown => break,
            Event::Hook(reply, hook) => {
                if let Err(err) = events::handle_event(&mut state, mp, reply) {
                    log::error!("hook `{}` failed: {err:?}", hook.name());
                }

                if let Err(err) = mp.hook_continue(hook.id()) {
                    log::error!("failed to continue hook `{}`: {:?}", hook.name(), err);
                }
            }
            Event::ClientMessage(data) => {
                if let ["key-binding", "watched", "d--" | "p--" | "dm-", ..] = data.args().as_slice()
                    && let Err(err) = events::mark_as_watched(&mut state, mp)
                {
                    log::error!("failed to mark as watched: {err:?}");
                }
            }
            Event::PropertyChange(reply, property) => {
                if let Err(err) = events::handle_event(&mut state, mp, reply) {
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
