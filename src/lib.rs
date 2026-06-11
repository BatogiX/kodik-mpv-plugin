use anyhow::Context;
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
fn main(mp: &Handle, event_token: EventQueueToken) -> i32 {
    match run(mp, event_token) {
        Ok(()) => 0,
        Err(err) => {
            log::error!("{err}");
            1
        }
    }
}

fn run(mp: &Handle, mut event_token: EventQueueToken) -> anyhow::Result<()> {
    let mut state = PluginState::new(mp).context("failed to initialize plugin state")?;
    let mut cache = Cache::load(mp).context("failed to load cache")?;
    events::register(mp).context("failed to register hooks")?;

    loop {
        match mp.wait_event(&mut event_token, -1.) {
            Event::Shutdown => break,
            Event::Hook(reply, hook) => {
                if let Err(err) = events::handle_hook(&mut state, mp, reply) {
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
                if let Err(err) = events::handle_property_change(&mut state, mp, reply, &property) {
                    log::error!("observe `{}` failed: {err:?}", property.name());
                }
            }
            event => println!("\n{event}\n"), // _ => {}
        }
    }

    cache.update_and_save().context("failed to update and save cache")?;

    Ok(())
}
