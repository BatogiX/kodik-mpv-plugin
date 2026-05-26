use anyhow::{Context as _, Result};
use base64::Engine as _;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use kodik_utils::GET as _;
use lazy_regex::{Lazy, Regex};
use mpv_client::{Handle, Node, Property};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::config::Quality;
use crate::mpv_ext::MpvExt;
use crate::shiki::ShikiMetaData;
use crate::state::PluginState;
use crate::{MpvResultExt as _, shiki};

pub const ON_LOAD_REPLY: u64 = 1;
pub const OBSERVE_VID_REPLY: u64 = 2;
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

    pub fn encode(&self, media_title: &str) -> Result<String> {
        let json = serde_json::to_vec(self).context("failed to serialize kodik payload")?;
        let encoded = BASE64_URL_SAFE_NO_PAD.encode(json);

        Ok(format!(
            "force-media-title={media_title},script-opts-append={KODIK_PAYLOAD_KEY}={encoded}"
        ))
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

    Ok(())
}

pub fn handle_hook(state: &mut PluginState, mpv: &mut Handle, reply: u64) -> Result<()> {
    match reply {
        ON_LOAD_REPLY => on_load(state, mpv),
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

            match payload.metadata_key.split_once('.').expect("expected host").0 {
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

    match payload.metadata_key.split_once('.').expect("expected host").0 {
        "shikimori" => shiki::mark_as_watched(state, mpv, payload),
        "myanimelist" => todo!(),
        "imdb" => todo!(),
        "kinopoisk" => todo!(),
        "mydramalist" => todo!(),
        _ => Ok(()),
    }?;

    Ok(())
}

pub fn handle_observe(state: &PluginState, mpv: &mut Handle, reply: u64, property: &Property) -> Result<()> {
    match reply {
        OBSERVE_VID_REPLY => observe_vid_reply(state, mpv, property),
        _ => Ok(()),
    }
}

fn observe_vid_reply(state: &PluginState, mpv: &mut Handle, _property: &Property) -> Result<()> {
    let current_pos: i64 = mpv.get_playlist_pos()?;

    if !is_fake_kodik_vid(mpv) {
        return Ok(());
    }

    let mut script_opts = mpv.get_script_opts()?;

    let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
        return Ok(());
    };

    let Node::String(payload_encoded) = node else {
        anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
    };

    let payload = Payload::decode(&payload_encoded)?;

    let kodik_videos = state
        .kodik_videos()
        .get(payload.metadata_key())
        .context("kodik videos should exist after on_load hook")?;

    let current_translation_title = mpv
        .get_property::<String>("current-tracks/video/title")
        .mpv_context("failed to get `current-tracks/video/title`")?;

    let result = match kodik_videos
        .results
        .iter()
        .find(|result| result.translation.title == current_translation_title)
        .context("no results found")
    {
        Ok(result) => result,
        Err(err) => {
            mpv.playlist_next_weak()?;
            return Err(err);
        }
    };

    let Some(indirect_link) = result.seasons.as_ref().map_or(Some(&result.link), |seasons| {
        seasons
            .iter()
            .last()
            .and_then(|(_, season)| season.episodes.get(&payload.episode()))
    }) else {
        mpv.playlist_next_weak()?;
        anyhow::bail!("episode not found");
    };

    let mut links = state.runtime().block_on(async {
        let links = kodik_parser::parse(state.client(), format!("https:{indirect_link}").as_str()).await?;
        Ok::<[String; 3], anyhow::Error>([links.p720, links.p480, links.p360])
    })?;

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

    match quality {
        Quality::P720 => {}
        Quality::P480 => links.swap(1, 0),
        Quality::P360 => links.swap(2, 0),
    }

    let mut direct_link = None;
    for link in links {
        let text = state
            .runtime()
            .block_on(async { state.client().fetch_as_text(&link).await })?;

        if text.is_empty() {
            continue;
        }

        direct_link = Some(link);
        break;
    }

    let Some(direct_link) = direct_link else {
        anyhow::bail!("invalid links")
    };

    let media_title = mpv
        .get_property::<String>("media-title")
        .mpv_context("failed to get `media-title`")?;

    mpv.loadfile_insert_at(
        direct_link.as_str(),
        &current_pos.to_string(),
        &payload.encode(&media_title)?,
    )?;

    mpv.playlist_remove(current_pos + 1)?;
    mpv.set_playlist_pos(&current_pos.to_string())?;

    Ok(())
}

fn is_fake_kodik_vid(mpv: &mut Handle) -> bool {
    let codec = mpv.get_property::<String>("current-tracks/video/codec").ok();
    let w = mpv
        .get_property::<i64>("current-tracks/video/demux-w")
        .unwrap_or_default();
    let h = mpv
        .get_property::<i64>("current-tracks/video/demux-h")
        .unwrap_or_default();
    let external = mpv
        .get_property::<bool>("current-tracks/video/external")
        .unwrap_or(false);

    external && codec.as_deref() == Some("vp9") && w == 16 && h == 16
}

pub fn file_loaded(state: &PluginState, mpv: &mut Handle) -> Result<()> {
    if is_fake_kodik_vid(mpv) {
        return Ok(());
    }

    let mut script_opts = mpv.get_script_opts()?;

    let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
        return Ok(());
    };

    let Node::String(payload_encoded) = node else {
        anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
    };

    let payload = Payload::decode(&payload_encoded)?;

    let kodik_videos = state
        .kodik_videos()
        .get(payload.metadata_key())
        .context("kodik videos should exist after on_load hook")?;

    for result in &kodik_videos.results {
        let title = &result.translation.title;
        mpv.video_add(LAZY_PLACEHOLDER_WEBM_B64, "auto", title)?;
    }

    Ok(())
}
