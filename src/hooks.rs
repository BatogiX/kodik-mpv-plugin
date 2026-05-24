use anyhow::{Context as _, Result};
use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use mpv_client::{Hook, Node};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::mpv_ext::MpvExt;
use crate::shiki::ShikiMetaData;
use crate::state::PluginState;
use crate::{MpvResultExt as _, shiki};

const ON_LOAD_REPLY: u64 = 1;
const ON_LOAD_PRIORITY: i32 = 50;
pub const KODIK_PAYLOAD_KEY: &str = "kodik-payload";

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum MetaData {
    Shiki(ShikiMetaData),
    Mal,
    Imdb,
    Kinopoisk,
    Mdl,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Payload {
    metadata_key: String,
    episode: usize,
}

impl Payload {
    pub const fn new(metadata_key: String, episode: usize) -> Self {
        Self { metadata_key, episode }
    }

    pub fn encode(&self) -> Result<String> {
        let json = serde_json::to_vec(self).context("failed to serialize kodik payload")?;
        let encoded = BASE64_URL_SAFE_NO_PAD.encode(json);

        Ok(format!("script-opts-append={KODIK_PAYLOAD_KEY}={encoded}"))
    }

    pub fn decode(encoded: &str) -> Result<Self> {
        let bytes = BASE64_URL_SAFE_NO_PAD
            .decode(encoded)
            .context("failed to decode kodik payload")?;

        serde_json::from_slice(&bytes).context("failed to deserialize kodik payload")
    }

    pub fn metadata_key(&self) -> &str {
        &self.metadata_key
    }

    pub const fn episode(&self) -> usize {
        self.episode
    }
}

pub fn register(state: &mut PluginState) -> Result<()> {
    state
        .mpv_mut()
        .hook_add(ON_LOAD_REPLY, "on_load", ON_LOAD_PRIORITY)
        .mpv_context("failed to add on_load hook")?;

    Ok(())
}

pub fn handle_hook(state: &mut PluginState, hook: &Hook) -> Result<()> {
    match hook.name() {
        "on_load" => on_load(state),
        _ => Ok(()),
    }
}

fn on_load(state: &mut PluginState) -> Result<()> {
    let filename = state.mpv_mut().get_stream_open_filename()?;

    let url = match Url::parse(&filename) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url,
        _ => {
            let mut script_opts = state.mpv_mut().get_script_opts()?;

            let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
                return Ok(());
            };

            let Node::String(payload_encoded) = node else {
                anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
            };

            let payload = Payload::decode(&payload_encoded)?;

            match payload.metadata_key.split_once('.').expect("expected host").0 {
                "shikimori" => shiki::on_load(state, &payload),
                "myanimelist" => todo!(),
                "imdb" => todo!(),
                "kinopoisk" => todo!(),
                "mydramalist" => todo!(),
                _ => Ok(()),
            }?;

            return Ok(());
        }
    };

    let Some(host) = url.host_str() else {
        return Ok(());
    };

    let Some(host_name) = host.rsplit_once('.').map(|(lp, _rp)| lp) else {
        return Ok(());
    };

    match host_name {
        "shikimori" => shiki::expand(state, url.as_str(), host),
        "myanimelist" => todo!(),
        "kinopoisk" => todo!(),
        "imdb" => todo!(),
        _ => Ok(()),
    }?;

    Ok(())
}

pub fn mark_as_watched(state: &mut PluginState) -> Result<()> {
    let mut script_opts = state.mpv_mut().get_script_opts()?;

    let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
        anyhow::bail!("missing `{KODIK_PAYLOAD_KEY}` in `script-opts`")
    };

    let Node::String(payload_encoded) = node else {
        anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
    };

    let payload = Payload::decode(&payload_encoded)?;

    match payload.metadata_key.split_once('.').expect("expected host").0 {
        "shikimori" => shiki::mark_as_watched(state, payload),
        "myanimelist" => todo!(),
        "imdb" => todo!(),
        "kinopoisk" => todo!(),
        "mydramalist" => todo!(),
        _ => Ok(()),
    }?;

    Ok(())
}
