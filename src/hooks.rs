use anyhow::{Context as _, Result};
use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use mpv_client::{Hook, Node};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::shiki::ShikiPayload;
use crate::state::PluginState;
use crate::{MpvResultExt as _, shiki};

const ON_LOAD_REPLY: u64 = 1;
const ON_LOAD_PRIORITY: i32 = 50;

#[derive(Debug, Serialize, Deserialize)]
pub enum Payload {
    Shiki(ShikiPayload),
    Mal,
    Imdb,
    Kinopoisk,
    Mdl,
}

impl Payload {
    pub fn encode(&self) -> Result<String> {
        let json = serde_json::to_vec(self).context("failed to serialize kodik payload")?;
        Ok(BASE64_URL_SAFE_NO_PAD.encode(json))
    }

    pub fn decode(encoded: &str) -> Result<Self> {
        let bytes = BASE64_URL_SAFE_NO_PAD
            .decode(encoded)
            .context("failed to decode kodik payload")?;

        serde_json::from_slice(&bytes).context("failed to deserialize kodik payload")
    }
}

#[derive(Debug)]
pub struct MpvFileOptions {
    pub title: Option<String>,
    pub payload: Payload,
}

impl MpvFileOptions {
    pub fn to_mpv_options_string(&self) -> Result<String> {
        let mut options = Vec::new();

        if let Some(title) = &self.title {
            options.push(format!("force-media-title={}", escape_mpv_option_value(title)));
        }

        let payload = &self.payload;
        let encoded = payload.encode()?;

        options.push(format!(
            "script-opts-append=kodik-payload={}",
            escape_mpv_option_value(&encoded)
        ));

        Ok(options.join(","))
    }
}

fn escape_mpv_option_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace(',', "\\,")
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
    let filename: String = state
        .mpv_mut()
        .get_property("stream-open-filename")
        .mpv_context("failed to get stream-open-filename")?;

    let url = match Url::parse(&filename) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url,
        _ => {
            let node: Node = state
                .mpv_mut()
                .get_property("options/script-opts")
                .mpv_context("failed to get script-opts")?;

            let Node::Map(mut script_opts) = node else {
                anyhow::bail!("`script-opts` is not a map")
            };

            let Some(kodik_payload) = script_opts.remove("kodik-payload") else {
                return Ok(());
            };

            let Node::String(kodik_payload) = kodik_payload else {
                anyhow::bail!("`kodik-payload` is not a string")
            };

            let kodik_payload = Payload::decode(&kodik_payload)?;
            match kodik_payload {
                Payload::Shiki(shiki_payload) => shiki::on_load(state, &shiki_payload),
                Payload::Mal => todo!(),
                Payload::Imdb => todo!(),
                Payload::Kinopoisk => todo!(),
                Payload::Mdl => todo!(),
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
        "myanimelist" => Ok(()),
        "kinopoisk" => Ok(()),
        "imdb" => Ok(()),
        _ => Ok(()),
    }?;

    Ok(())
}

pub fn mark_as_watched(state: &mut PluginState) -> Result<()> {
    let node: Node = state
        .mpv_mut()
        .get_property("options/script-opts")
        .mpv_context("failed to get script-opts")?;

    let Node::Map(mut script_opts) = node else {
        anyhow::bail!("`script-opts` is not a map")
    };

    let Some(payload) = script_opts.remove("kodik-payload") else {
        anyhow::bail!("missing `kodik-payload` in `script-opts`")
    };

    let Node::String(payload) = payload else {
        anyhow::bail!("`kodik-payload` is not a string")
    };

    let payload = Payload::decode(&payload)?;
    match payload {
        Payload::Shiki(shiki_payload) => shiki::mark_as_watched(state, &shiki_payload),
        Payload::Mal => todo!(),
        Payload::Imdb => todo!(),
        Payload::Kinopoisk => todo!(),
        Payload::Mdl => todo!(),
    }?;

    Ok(())
}
