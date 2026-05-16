use std::sync::Arc;

use anyhow::{Context as _, Result};
use mpv_client::{Handle, mpv_handle};
use reqwest::Client;
use tokio::runtime::{Builder, Runtime};

use crate::{config::Config, logger};

pub struct PluginState<'a> {
    mpv: &'a mut Handle,
    client: Client,
    runtime: Runtime,
    config: Config,
}

impl PluginState<'_> {
    pub fn new(handle: *mut mpv_handle) -> Result<Self> {
        let mpv = Handle::from_ptr(handle);
        let config = Config::load()?;

        logger::init_logger(mpv.name(), config.log_level());

        let client = Client::builder()
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .zstd(true)
            .deflate(true)
            .cookie_provider(Arc::new(config.load_cookies()?))
            .build()?;

        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to create tokio runtime")?;

        Ok(Self {
            mpv,
            client,
            runtime,
            config,
        })
    }

    pub const fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub const fn client(&self) -> &Client {
        &self.client
    }

    pub const fn mpv_mut(&mut self) -> &mut Handle {
        &mut *self.mpv
    }

    pub const fn config(&self) -> &Config {
        &self.config
    }
}
