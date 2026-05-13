use std::collections::HashSet;

use anyhow::{Context as _, Result};
use mpv_client::{Handle, mpv_handle};
use reqwest::Client;
use tokio::runtime::{Builder, Runtime};

pub struct PluginState<'a> {
    mpv: &'a mut Handle,
    client: Client,
    runtime: Runtime,
    expanded_playlist_urls: HashSet<String>,
}

impl PluginState<'_> {
    pub fn new(handle: *mut mpv_handle) -> Result<Self> {
        let mpv = Handle::from_ptr(handle);
        let client = Client::builder()
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .zstd(true)
            .deflate(true)
            .build()?;

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to create tokio runtime")?;

        Ok(Self {
            mpv,
            client,
            runtime,
            expanded_playlist_urls: HashSet::new(),
        })
    }

    pub const fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub fn is_expanded(&self, url: &str) -> bool {
        self.expanded_playlist_urls.contains(url)
    }

    pub fn mark_expanded<I, S>(&mut self, urls: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for url in urls {
            self.expanded_playlist_urls.insert(url.into());
        }
    }

    pub const fn client(&self) -> &Client {
        &self.client
    }

    pub const fn mpv_mut(&mut self) -> &mut Handle {
        &mut *self.mpv
    }
}
