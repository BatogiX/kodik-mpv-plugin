use std::ffi::c_int;

use mpv_client::{Event, mpv_handle};

mod hooks;
mod mpv_ext;
mod related;
mod state;
mod ytdl;

use crate::mpv_ext::MpvResultExt;
use crate::state::PluginState;

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "C" fn mpv_open_cplugin(handle: *mut mpv_handle) -> c_int {
    let mut state = match PluginState::new(handle) {
        Ok(state) => state,
        Err(err) => {
            eprintln!("failed to initialize plugin state: {err:?}");
            return 1;
        }
    };

    if let Err(err) = hooks::register(&mut state) {
        eprintln!("failed to register hooks: {err:?}");
        return 1;
    }

    if let Err(err) = ytdl::configure_ytdl_excludes(&mut state) {
        eprintln!("failed to configure ytdl excludes: {err:?}");
        return 1;
    }

    loop {
        match state.mpv_mut().wait_event(-1.) {
            Event::Shutdown => return 0,
            Event::Hook(_, hook) => {
                if let Err(err) = hooks::handle_hook(&mut state, &hook) {
                    eprintln!("hook failed: {err:?}");
                }

                if let Err(err) = state.mpv_mut().hook_continue(hook.id()) {
                    eprintln!("failed to continue hook {}: {:?}", hook.id(), err);
                }
            }
            _ => {}
        }
    }
}
