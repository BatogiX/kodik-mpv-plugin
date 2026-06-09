use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use anyhow::{Result, anyhow};
use mpv_client::{Format, Handle, Node};

pub trait MpvResultExt<T> {
    fn mpv_context<'a, S: Into<Cow<'a, str>>>(self, context: S) -> Result<T>;
    fn with_mpv_context<'a, F, S>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<Cow<'a, str>>;
}

impl<T> MpvResultExt<T> for std::result::Result<T, mpv_client::Error> {
    fn mpv_context<'a, S: Into<Cow<'a, str>>>(self, context: S) -> Result<T> {
        let context = context.into();
        self.map_err(|err| anyhow!("{context}: {err}"))
    }

    fn with_mpv_context<'a, F, S>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<Cow<'a, str>>,
    {
        self.map_err(|err| {
            let context = f().into();
            anyhow!("{context}: {err}")
        })
    }
}

pub trait MpvExt {
    fn get_script_opts(&self) -> Result<HashMap<String, Node>>;
    fn get_stream_open_filename(&self) -> Result<String>;
    fn set_stream_open_filename<S: Into<String>>(&self, filename: S) -> Result<()>;
    fn playlist_next_weak(&self) -> Result<()>;
    fn expand_path(&self, path: &str) -> Result<PathBuf>;
    fn playlist_remove(&self, index: i64) -> Result<()>;
    fn loadfile_insert_at(&self, url: &str, index: &str, options: &str) -> Result<()>;
    fn get_playlist_pos(&self) -> Result<i64>;
    fn set_playlist_pos(&self, pos: &str) -> Result<()>;
    fn playlist_play_index(&self, index: &str) -> Result<()>;
    fn get_ytdl_format(&self) -> Result<String>;
    fn audio_add(&self, url: &str, flag: &str, title: &str) -> Result<()>;
    fn hook_add_ext(&self, reply: u64, name: &str, priority: i32) -> Result<()>;
    fn observe_property_ext<'a, S: Into<Cow<'a, str>>, T: Format>(&self, reply: u64, name: S) -> Result<()>;
    fn set_file_local_options_start<S: ToString>(&self, time_pos: S) -> Result<()>;
    fn get_time_pos(&self) -> Result<f64>;
    fn reload_current_file(&self) -> Result<()>;
    fn get_current_tracks_audio_title(&self) -> Result<String>;
    fn get_playlist_filename_by_index(&self, index: i64) -> Result<String>;
    fn get_playlist_count(&self) -> Result<i64>;
}

impl MpvExt for Handle {
    fn get_script_opts(&self) -> Result<HashMap<String, Node>> {
        let node = self
            .get_property("options/script-opts")
            .mpv_context("failed to `get-property script-opts`")?;

        let Node::Map(script_opts) = node else {
            anyhow::bail!("`script-opts` is not a map")
        };

        anyhow::Ok(script_opts)
    }

    fn get_stream_open_filename(&self) -> Result<String> {
        self.get_property("stream-open-filename")
            .mpv_context("failed to `get-property stream-open-filename`")
    }

    fn set_stream_open_filename<S: Into<String>>(&self, filename: S) -> Result<()> {
        let filename = filename.into();
        self.set_property("stream-open-filename", filename.clone())
            .with_mpv_context(|| format!("failed to `set-property stream-open-filename {filename}`"))
    }

    fn playlist_next_weak(&self) -> Result<()> {
        self.command(["playlist-next", "weak"])
            .mpv_context("failed to `playlist-next weak`")
    }

    fn expand_path(&self, path: &str) -> Result<PathBuf> {
        let node = self
            .command_ret(["expand-path", path])
            .with_mpv_context(|| format!("failed to `expand-path {path}`"))?;

        let Node::String(expanded_path) = node else {
            anyhow::bail!("`expand-path {path}` returned non-string value");
        };

        anyhow::Ok(PathBuf::from(expanded_path))
    }

    fn playlist_remove(&self, index: i64) -> Result<()> {
        self.command(["playlist-remove", &index.to_string()])
            .with_mpv_context(|| format!("failed to `playlist-remove {index}`"))
    }

    fn loadfile_insert_at(&self, url: &str, index: &str, options: &str) -> Result<()> {
        self.command(["loadfile", url, "insert-at", index, options])
            .with_mpv_context(|| format!("failed to `loadfile {url} insert-at {index} {options}`"))
    }

    fn get_playlist_pos(&self) -> Result<i64> {
        self.get_property("playlist-pos")
            .mpv_context("failed to `get-property playlist-pos`")
    }

    fn set_playlist_pos(&self, pos: &str) -> Result<()> {
        self.command(["set", "playlist-pos", pos])
            .with_mpv_context(|| format!("failed to `set playlist-pos {pos}`"))
    }

    fn playlist_play_index(&self, index: &str) -> Result<()> {
        self.command(["playlist-play-index", index])
            .with_mpv_context(|| format!("failed to `playlist-play-index {index}`"))
    }

    fn get_ytdl_format(&self) -> Result<String> {
        self.get_property("ytdl-format")
            .mpv_context("failed to `get-property ytdl-format`")
    }

    fn video_add(&self, url: &str, flag: &str, title: &str) -> Result<()> {
        self.command(["video-add", url, flag, title, "ru"])
            .with_mpv_context(|| format!("failed to `video-add {url} {flag} {title} ru`"))
    }

    fn hook_add_ext(&self, reply: u64, name: &str, priority: i32) -> Result<()> {
        self.hook_add(reply, name, priority)
            .with_mpv_context(|| format!("failed to `hook-add {reply} {name} {priority}`"))
    }

    fn observe_property_ext<'a, S: Into<Cow<'a, str>>, T: Format>(&self, reply: u64, name: S) -> Result<()> {
        let name = name.into();
        self.observe_property::<&str, T>(reply, name.as_ref())
            .with_mpv_context(|| format!("failed to `observe-property {name}`"))
    }

    fn set_file_local_options_start<S: ToString>(&self, time_pos: S) -> Result<()> {
        let time_pos = time_pos.to_string();
        self.set_property("file-local-options/start", time_pos.clone())
            .with_mpv_context(|| format!("failed to `set-property file-local-options/start {time_pos}`"))
    }

    fn get_time_pos(&self) -> Result<f64> {
        self.get_property("time-pos")
            .mpv_context("failed to `get-property time-pos`")
    }

    fn reload_current_file(&self) -> Result<()> {
        self.command(["playlist-play-index", "current", "yes"])
            .mpv_context("failed to `playlist-play-index current yes`")
    }

    fn get_current_tracks_audio_title(&self) -> Result<String> {
        self.get_property::<&str, String>("current-tracks/audio/title")
            .mpv_context("failed to `get-property current-tracks/audio/title`")
    }

    fn get_playlist_filename_by_index(&self, index: i64) -> Result<String> {
        self.get_property(format!("playlist/{index}/filename"))
            .with_mpv_context(|| format!("failed to get `playlist/{index}/filename`"))
    }

    fn get_playlist_count(&self) -> Result<i64> {
        self.get_property("playlist-count")
            .mpv_context("failed to `get-property playlist-count`")
    }
}
