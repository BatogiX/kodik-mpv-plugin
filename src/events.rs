use anyhow::{Context as _, Result};
use lazy_regex::{Lazy, Regex, regex};
use mpv_client::{Handle, Node};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::config::Quality;
use crate::mpv_ext::MpvExt;
use crate::shiki;
use crate::shiki::ShikiMetaData;
use crate::state::PluginState;

const ON_LOAD_REPLY: u64 = 0;
const ON_PRELOADED_REPLY: u64 = 1;
const OBSERVE_VID_REPLY: u64 = 2;
const OBSERVE_YTDL_FORMAT_REPLY: u64 = 3;
const ON_LOAD_PRIORITY: i32 = 50;
const ON_PRELOADED_PRIORITY: i32 = 50;
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
        let json = serde_json::to_string(self).context("failed to serialize kodik payload")?;

        let script_opt = format!("{KODIK_PAYLOAD_KEY}={json}");
        let quoted = format!("%{}%{}", script_opt.len(), script_opt);

        Ok(format!("script-opts-append={quoted}"))
    }

    pub fn decode(raw_json: &str) -> Result<Self> {
        serde_json::from_str(raw_json).context("failed to deserialize kodik payload")
    }

    pub fn metadata_key(&self) -> &str {
        &self.metadata_key
    }

    pub const fn episode(&self) -> usize {
        self.episode
    }
}

pub fn register(mpv: &mut Handle) -> Result<()> {
    mpv.hook_add_ext(ON_LOAD_REPLY, "on_load", ON_LOAD_PRIORITY)?;
    mpv.hook_add_ext(ON_PRELOADED_REPLY, "on_preloaded", ON_PRELOADED_PRIORITY)?;
    mpv.observe_property_ext::<i64>(OBSERVE_VID_REPLY, "current-tracks/video/id")?;
    mpv.observe_property_ext::<String>(OBSERVE_YTDL_FORMAT_REPLY, "ytdl-format")?;

    Ok(())
}

pub fn handle_event(state: &mut PluginState, mpv: &mut Handle, reply: u64) -> Result<()> {
    match reply {
        ON_LOAD_REPLY => on_load(state, mpv),
        ON_PRELOADED_REPLY => on_preloaded(state, mpv),
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
        "shikimori" => shiki::mark_as_watched(state, mpv, &payload),
        "myanimelist" => todo!(),
        "imdb" => todo!(),
        "kinopoisk" => todo!(),
        "mydramalist" => todo!(),
        _ => Ok(()),
    }?;

    Ok(())
}

fn on_preloaded(state: &PluginState, mpv: &mut Handle) -> Result<()> {
    const VIDEO_TRACK_PLACEHOLDER: &str = "av://lavfi:color=c=black@0.0:s=1280x720:r=1";

    let mut script_opts = mpv.get_script_opts()?;

    let Some(node) = script_opts.remove(KODIK_PAYLOAD_KEY) else {
        return Ok(());
    };

    let Node::String(payload_encoded) = node else {
        anyhow::bail!("`{KODIK_PAYLOAD_KEY}` is not a string")
    };

    let payload = Payload::decode(&payload_encoded)?;
    let (metadata_key, episode) = (payload.metadata_key(), payload.episode());

    let kodik_videos = state
        .kodik_videos()
        .get(metadata_key)
        .context("kodik videos should exist after on-load hook")?;

    for result in &kodik_videos.results {
        let mut episodes_accum = 0;
        let title = &result.translation.title;

        let found_season = if let Some(seasons) = &result.seasons {
            let mut found = None;
            for (_, season) in seasons.iter().filter(|(number, _)| **number > 0) {
                let last_episode = season.episodes.last_key_value().context("season must have episodes")?.0;
                if (episodes_accum + last_episode) < episode {
                    episodes_accum += last_episode;
                } else {
                    found = Some(season);
                    break;
                }
            }
            found
        } else {
            None
        };

        if let Some(season) = found_season {
            if !season.episodes.contains_key(&(episode - episodes_accum)) {
                continue;
            }
        } else if episode > 1 {
            continue;
        }

        mpv.video_add(VIDEO_TRACK_PLACEHOLDER, "auto", title)?;
    }

    Ok(())
}

fn observe_vid_reply(state: &mut PluginState, mpv: &mut Handle) -> Result<()> {
    let Some(_) = mpv.get_script_opts()?.get(KODIK_PAYLOAD_KEY) else {
        return Ok(());
    };

    let Ok(current_translation_title) = mpv.get_current_tracks_video_title() else {
        return Ok(());
    };

    state
        .config_mut()
        .set_translation_title(Some(regex::escape(&current_translation_title)));

    let time_pos = mpv.get_time_pos()?;
    mpv.set_file_local_options_start(time_pos)?;
    mpv.reload_current_file()?;

    Ok(())
}

fn observe_ytdl_format_reply(state: &mut PluginState, mpv: &mut Handle) -> Result<()> {
    const EXTRACT_HEIGHT_PATTERN: &Lazy<Regex> = lazy_regex::regex!(r"height<=\??(\d+)");

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
