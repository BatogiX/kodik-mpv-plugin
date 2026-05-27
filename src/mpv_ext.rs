use std::{collections::HashMap, fmt::Display, path::PathBuf};

use anyhow::{Result, anyhow};
use mpv_client::{Format, Handle, Node};

pub trait MpvResultExt<T> {
    fn mpv_context(self, context: impl Into<String>) -> Result<T>;
    fn with_mpv_context<F, C>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> C,
        C: Into<String>;
}

impl<T> MpvResultExt<T> for std::result::Result<T, mpv_client::Error> {
    fn mpv_context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|err| anyhow!("{}: {:?}", context.into(), err))
    }

    fn with_mpv_context<F, C>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> C,
        C: Into<String>,
    {
        self.map_err(|err| {
            let context = f().into();
            anyhow!("{context}: {err:?}")
        })
    }
}

pub trait MpvExt {
    fn get_script_opts(&mut self) -> Result<HashMap<String, Node>>;
    fn get_stream_open_filename(&mut self) -> Result<String>;
    fn set_stream_open_filename<S: Into<String>>(&mut self, filename: S) -> Result<()>;
    fn playlist_next_weak(&mut self) -> Result<()>;
    fn expand_path(&mut self, path: &str) -> Result<PathBuf>;
    fn playlist_remove(&mut self, index: i64) -> Result<()>;
    fn loadfile_insert_at(&mut self, url: &str, index: &str, options: &str) -> Result<()>;
    fn get_playlist_pos(&mut self) -> Result<i64>;
    fn set_playlist_pos(&mut self, pos: &str) -> Result<()>;
    fn playlist_play_index(&mut self, index: &str) -> Result<()>;
    fn get_ytdl_format(&mut self) -> Result<String>;
    fn video_add(&mut self, url: &str, flag: &str, title: &str) -> Result<()>;
    fn hook_add_ext(&mut self, reply: u64, name: &str, priority: i32) -> Result<()>;
    fn observe_property_ext<T: Format>(&mut self, reply: u64, name: impl AsRef<str>) -> Result<()>;
    fn set_file_local_options_start<S: ToString>(&mut self, time_pos: S) -> Result<()>;
    fn get_time_pos(&mut self) -> Result<f64>;
    fn reload_current_file(&mut self) -> Result<()>;
    fn get_current_tracks_video_title(&mut self) -> Result<String>;
    fn get_playlist_filename_by_index(&mut self, index: i64) -> Result<String>;
}

impl MpvExt for Handle {
    fn get_script_opts(&mut self) -> Result<HashMap<String, Node>> {
        let node = self
            .get_property("options/script-opts")
            .mpv_context("failed to `get-property script-opts`")?;

        let Node::Map(script_opts) = node else {
            anyhow::bail!("`script-opts` is not a map")
        };

        anyhow::Ok(script_opts)
    }

    fn get_stream_open_filename(&mut self) -> Result<String> {
        self.get_property("stream-open-filename")
            .mpv_context("failed to `get-property stream-open-filename`")
    }

    fn set_stream_open_filename<S: Into<String>>(&mut self, filename: S) -> Result<()> {
        let filename = filename.into();
        self.set_property("stream-open-filename", filename.clone())
            .with_mpv_context(|| format!("failed to `set-property stream-open-filename {filename}`"))
    }

    fn playlist_next_weak(&mut self) -> Result<()> {
        self.command(["playlist-next", "weak"])
            .mpv_context("failed to `playlist-next weak`")
    }

    fn expand_path(&mut self, path: &str) -> Result<PathBuf> {
        let node = self
            .command_ret(["expand-path", path])
            .with_mpv_context(|| format!("failed to `expand-path {path}`"))?;

        let Node::String(expanded_path) = node else {
            anyhow::bail!("`expand-path {path}` returned non-string value");
        };

        anyhow::Ok(PathBuf::from(expanded_path))
    }

    fn playlist_remove(&mut self, index: i64) -> Result<()> {
        self.command(["playlist-remove", &index.to_string()])
            .with_mpv_context(|| format!("failed to `playlist-remove {index}`"))
    }

    fn loadfile_insert_at(&mut self, url: &str, index: &str, options: &str) -> Result<()> {
        self.command(["loadfile", url, "insert-at", index, options])
            .with_mpv_context(|| format!("failed to `loadfile {url} insert-at {index} {options}`"))
    }

    fn get_playlist_pos(&mut self) -> Result<i64> {
        self.get_property("playlist-pos")
            .mpv_context("failed to `get-property playlist-pos`")
    }

    fn set_playlist_pos(&mut self, pos: &str) -> Result<()> {
        self.command(["set", "playlist-pos", pos])
            .with_mpv_context(|| format!("failed to `set playlist-pos {pos}`"))
    }

    fn playlist_play_index(&mut self, index: &str) -> Result<()> {
        self.command(["playlist-play-index", index])
            .with_mpv_context(|| format!("failed to `playlist-play-index {index}`"))
    }

    fn get_ytdl_format(&mut self) -> Result<String> {
        self.get_property("ytdl-format")
            .mpv_context("failed to `get-property ytdl-format`")
    }

    fn video_add(&mut self, url: &str, flag: &str, title: &str) -> Result<()> {
        self.command(["video-add", url, flag, title, "ru"])
            .with_mpv_context(|| format!("failed to `video-add {url} {flag} {title} ru`"))
    }

    fn hook_add_ext(&mut self, reply: u64, name: &str, priority: i32) -> Result<()> {
        self.hook_add(reply, name, priority)
            .with_mpv_context(|| format!("failed to `hook-add {reply} {name} {priority}`"))
    }

    fn observe_property_ext<T: Format>(&mut self, reply: u64, name: impl AsRef<str>) -> Result<()> {
        self.observe_property::<T>(reply, &name)
            .with_mpv_context(|| format!("failed to `observe-property {}`", name.as_ref()))
    }

    fn set_file_local_options_start<S: ToString>(&mut self, time_pos: S) -> Result<()> {
        let time_pos = time_pos.to_string();
        self.set_property("file-local-options/start", time_pos.clone())
            .with_mpv_context(|| format!("failed to `set-property file-local-options/start {time_pos}`"))
    }

    fn get_time_pos(&mut self) -> Result<f64> {
        self.get_property("time-pos")
            .mpv_context("failed to `get-property time-pos`")
    }

    fn reload_current_file(&mut self) -> Result<()> {
        self.command(["playlist-play-index", "current", "yes"])
            .mpv_context("failed to `playlist-play-index current yes`")
    }

    fn get_current_tracks_video_title(&mut self) -> Result<String> {
        self.get_property::<String>("current-tracks/video/title")
            .mpv_context("failed to `get-property current-tracks/video/title`")
    }

    fn get_playlist_filename_by_index(&mut self, index: i64) -> Result<String> {
        self.get_property(format!("playlist/{index}/filename"))
            .with_mpv_context(|| format!("failed to get `playlist/{index}/filename`"))
    }
}
