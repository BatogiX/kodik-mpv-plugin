use std::{collections::HashMap, path::PathBuf};

use anyhow::{Result, anyhow};
use mpv_client::{Handle, Node};

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
    fn set_stream_open_filename(&mut self, filename: impl Into<String>) -> Result<()>;
    fn playlist_next_weak(&mut self) -> Result<()>;
    fn expand_path(&mut self, path: &str) -> Result<PathBuf>;
    fn playlist_remove(&mut self, index: i64) -> Result<()>;
    fn loadfile_insert_at(&mut self, url: &str, index: &str, options: &str) -> Result<()>;
    fn get_playlist_pos(&mut self) -> Result<i64>;
    fn set_playlist_pos(&mut self, pos: &str) -> Result<()>;
    fn playlist_play_index(&mut self, index: &str) -> Result<()>;
    fn get_ytdl_format(&mut self) -> Result<String>;
    fn video_add(&mut self, url: &str, flag: &str, title: &str) -> Result<()>;
    fn video_add_async(&mut self, url: &str, flag: &str, title: &str) -> Result<()>;
}

impl MpvExt for Handle {
    fn get_script_opts(&mut self) -> Result<HashMap<String, Node>> {
        let node = self
            .get_property("options/script-opts")
            .mpv_context("failed to get script-opts")?;

        let Node::Map(script_opts) = node else {
            anyhow::bail!("`script-opts` is not a map")
        };

        anyhow::Ok(script_opts)
    }

    fn get_stream_open_filename(&mut self) -> Result<String> {
        self.get_property("stream-open-filename")
            .mpv_context("failed to get `stream-open-filename`")
    }

    fn set_stream_open_filename(&mut self, filename: impl Into<String>) -> Result<()> {
        self.set_property("stream-open-filename", filename.into())
            .mpv_context("failed to set `stream-open-filename`")
    }

    fn playlist_next_weak(&mut self) -> Result<()> {
        self.command(["playlist-next", "weak"])
            .mpv_context("failed to `playlist-next weak`")
    }

    fn expand_path(&mut self, path: &str) -> Result<PathBuf> {
        let node = self
            .command_ret(["expand-path", path])
            .with_mpv_context(|| format!("failed to expand-path `{path}`"))?;

        let Node::String(expanded_path) = node else {
            anyhow::bail!("`expand-path \"{path}\"` returned non-string value");
        };

        anyhow::Ok(PathBuf::from(expanded_path))
    }

    fn playlist_remove(&mut self, index: i64) -> Result<()> {
        self.command(["playlist-remove", &index.to_string()])
            .mpv_context("failed to `playlist-remove`")
    }

    fn loadfile_insert_at(&mut self, url: &str, index: &str, options: &str) -> Result<()> {
        self.command(["loadfile", url, "insert-at", index, options])
            .mpv_context("failed to insert-at file")
    }

    fn get_playlist_pos(&mut self) -> Result<i64> {
        self.get_property("playlist-pos")
            .mpv_context("failed to get current playlist position")
    }

    fn set_playlist_pos(&mut self, pos: &str) -> Result<()> {
        self.command(["set", "playlist-pos", pos])
            .mpv_context("failed to set playlist position")
    }

    fn playlist_play_index(&mut self, index: &str) -> Result<()> {
        self.command(["playlist-play-index", index])
            .mpv_context("failed to `playlist-play-index`")
    }

    fn get_ytdl_format(&mut self) -> Result<String> {
        self.get_property("ytdl-format")
            .mpv_context("failed to get `ytdl-format`")
    }

    fn video_add(&mut self, url: &str, flag: &str, title: &str) -> Result<()> {
        self.command(["video-add", url, flag, title, "ru"])
            .mpv_context("failed to `video-add`")
    }

    fn video_add_async(&mut self, url: &str, flag: &str, title: &str) -> Result<()> {
        self.command_async(0, ["video-add", url, flag, title, "ru"])
            .mpv_context("failed to `video-add`")
    }
}
