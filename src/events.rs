use anyhow::{Context as _, Result};
use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use lazy_regex::{Lazy, Regex};
use mpv_client::{Handle, Node};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::config::Quality;
use crate::mpv_ext::{MpvExt, MpvResultExt};
use crate::shiki;
use crate::shiki::ShikiMetaData;
use crate::state::PluginState;

pub const ON_LOAD_REPLY: u64 = 0;
pub const OBSERVE_VID_REPLY: u64 = 1;
pub const OBSERVE_YTDL_FORMAT_REPLY: u64 = 2;
const ON_LOAD_PRIORITY: i32 = 50;
pub const KODIK_PAYLOAD_KEY: &str = "kodik-payload";
pub const EXTRACT_HEIGHT_PATTERN: &Lazy<Regex> = lazy_regex::regex!(r"height<=\??(\d+)");
pub const LAZY_PLACEHOLDER_WEBM_B64: &str = "data://video/webm;base64,\
GkXfo59ChoEBQveBAULygQRC84EIQoKEd2VibUKHgQJChYECGFOAZwEAAAAAAAIGEU2bdLpNu4tTq4QVSalmU6yBoU27i1OrhBZUrmtTrIHWTbuMU6uEElTDZ1OsggEjTbuMU6uEHFO7a1OsggHw7AEAAAAAAABZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAVSalmsCrXsYMPQkBNgIxMYXZmNjEuNy4xMDBXQYxMYXZmNjEuNy4xMDBEiYhAj0AAAAAAABZUrmvIrgEAAAAAAAA/14EBc8WIHkCBGSwrtlWcgQAitZyDdW5kiIEAhoVWX1ZQOYOBASPjg4Q7msoA4JCwgRC6gRCagQJVsIRVuYEBElTDZ0B/c3OfY8CAZ8iZRaOHRU5DT0RFUkSHjExhdmY2MS43LjEwMHNz2mPAi2PFiB5AgRksK7ZVZ8ilRaOHRU5DT0RFUkSHmExhdmM2MS4xOS4xMDEgbGlidnB4LXZwOWfIoUWjiERVUkFUSU9ORIeTMDA6MDA6MDEuMDAwMDAwMDAwAB9DtnXD54EAo76BAACAgkmDQgAA8AD2BjgkHBhCAAAgQAAim///pRP7gpTejxVKt20n/VT9Ge2UUtNx0mzI/qb3ZUy8JgyAABxTu2uRu4+zgQC3iveBAfGCAajwgQM=";

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

pub fn register(mpv: &mut Handle) -> Result<()> {
    mpv.hook_add(ON_LOAD_REPLY, "on_load", ON_LOAD_PRIORITY)
        .mpv_context("failed to add on_load hook")?;

    mpv.observe_property::<i64>(OBSERVE_VID_REPLY, "current-tracks/video/id")
        .mpv_context("failed to observe property `current-tracks/video/id`")?;

    mpv.observe_property::<String>(OBSERVE_YTDL_FORMAT_REPLY, "ytdl-format")
        .mpv_context("failed to observe property `ytdl-format`")?;

    Ok(())
}

pub fn handle_event(state: &mut PluginState, mpv: &mut Handle, reply: u64) -> Result<()> {
    match reply {
        ON_LOAD_REPLY => on_load(state, mpv),
        OBSERVE_VID_REPLY => observe_vid_reply(state, mpv),
        OBSERVE_YTDL_FORMAT_REPLY => observe_ytdl_format_reply(state, mpv),
        _ => Ok(()),
    }
}

fn on_load(state: &mut PluginState, mpv: &mut Handle) -> Result<()> {
    let filename = mpv.get_stream_open_filename()?;

    let url = match Url::parse(&filename) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url,
        _ => {
            let mut script_opts = mpv.get_script_opts()?;

            let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
                return Ok(());
            };

            let Node::String(payload_encoded) = node else {
                anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
            };

            let payload = Payload::decode(&payload_encoded)?;

            match payload.metadata_key.split_once('.').context("expected host")?.0 {
                "shikimori" => shiki::on_load(state, mpv, &payload),
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
        "shikimori" => shiki::expand(state, mpv, url.as_str(), host),
        "myanimelist" => todo!(),
        "kinopoisk" => todo!(),
        "imdb" => todo!(),
        _ => Ok(()),
    }?;

    Ok(())
}

pub fn mark_as_watched(state: &mut PluginState, mpv: &mut Handle) -> Result<()> {
    let mut script_opts = mpv.get_script_opts()?;

    let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
        anyhow::bail!("missing `{KODIK_PAYLOAD_KEY}` in `script-opts`")
    };

    let Node::String(payload_encoded) = node else {
        anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
    };

    let payload = Payload::decode(&payload_encoded)?;

    match payload.metadata_key.split_once('.').context("expected host")?.0 {
        "shikimori" => shiki::mark_as_watched(state, mpv, payload),
        "myanimelist" => todo!(),
        "imdb" => todo!(),
        "kinopoisk" => todo!(),
        "mydramalist" => todo!(),
        _ => Ok(()),
    }?;

    Ok(())
}

fn observe_vid_reply(state: &mut PluginState, mpv: &mut Handle) -> Result<()> {
    let Some(_) = mpv.get_script_opts()?.remove(KODIK_PAYLOAD_KEY) else {
        return Ok(());
    };

    let current_vid = mpv
        .get_property::<i64>("vid")
        .mpv_context("failed to `get-property vid`")?;

    let original_vid = get_original_vid(mpv)?;

    if current_vid == original_vid {
        return Ok(());
    }

    if original_vid == -1 {
        return Ok(());
    }

    let current_translation_title = mpv
        .get_property::<String>("current-tracks/video/title")
        .mpv_context("failed to get `current-tracks/video/title`")?;

    state
        .config_mut()
        .set_translation_title(Some(current_translation_title));

    let time_pos: f64 = mpv
        .get_property("time-pos")
        .mpv_context("failed to `get-property time-pos`")?;

    mpv.set_property("file-local-options/start", time_pos.to_string())
        .with_mpv_context(|| format!("failed to `set-property file-local-options/start {time_pos}`"))?;

    mpv.set_property("vid", original_vid)
        .with_mpv_context(|| format!("failed to `set-property vid {original_vid}`"))?;

    mpv.command(["playlist-play-index", "current", "yes"])
        .with_mpv_context(|| "failed to reload current file".to_string())?;

    Ok(())
}

fn get_original_vid(mpv: &mut Handle) -> Result<i64> {
    let count: i64 = mpv
        .get_property("track-list/count")
        .mpv_context("failed to `get-property track-list/count`")?;

    for i in 0..count {
        let external = mpv
            .get_property::<bool>(format!("track-list/{i}/external"))
            .unwrap_or(false);

        if external {
            continue;
        }

        let id: i64 = mpv
            .get_property(format!("track-list/{i}/id"))
            .with_mpv_context(|| format!("failed to `get-property track-list/{i}/id`"))?;

        return Ok(id);
    }

    Ok(-1)
}

fn observe_ytdl_format_reply(state: &mut PluginState, mpv: &mut Handle) -> Result<()> {
    let quality = EXTRACT_HEIGHT_PATTERN
        .captures(&mpv.get_ytdl_format()?)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i32>().ok())
        .map_or_else(
            || state.config().quality(),
            |height| match height {
                h if h >= 720 => Quality::P720,
                480 => Quality::P480,
                h if h <= 360 => Quality::P360,
                _ => state.config().quality(),
            },
        );

    state.config_mut().set_quality(quality);

    Ok(())
}
