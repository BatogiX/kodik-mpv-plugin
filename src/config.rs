use anyhow::{Context as _, Result};
use kodik_parser::TranslationType;
use log::LevelFilter;
use mpv_client::{Handle, Node};
use reqwest::{Url, cookie::Jar};
use std::{
    borrow::ToOwned,
    collections::HashMap,
    fs::{self, File},
    io::{BufRead as _, BufReader},
    path::PathBuf,
    str::FromStr,
    string::String,
};

use crate::mpv_ext::MpvExt;

const CONFIG_PATH: &str = "~~home/script-opts/kodik.conf";

#[derive(Debug, Clone, Copy)]
pub enum Quality {
    P720,
    P480,
    P360,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelatedMode {
    All,
    Essential,
    None,
}

#[derive(Debug)]
pub struct Config {
    /// Specify video quality [default: 720] [possible values: 360, 480, 720]
    quality: Quality,

    /// Netscape formatted file to read cookies from
    cookies: Option<PathBuf>,

    /// Specify translation title
    translation_title: Option<String>,

    /// Specify translation type [possible values: voice, subtitles]
    translation_type: Option<TranslationType>,

    /// Expand a media database URL into all related URLs [possible values: all, essential, none]
    related_mode: RelatedMode,

    log_level: LevelFilter,
}

impl FromStr for Quality {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim() {
            "360" => Ok(Self::P360),
            "480" => Ok(Self::P480),
            "720" => Ok(Self::P720),
            value => anyhow::bail!("invalid `quality`: `{value}`, expected 360, 480, or 720"),
        }
    }
}

impl FromStr for RelatedMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim() {
            "all" => Ok(Self::All),
            "essential" => Ok(Self::Essential),
            "none" => Ok(Self::None),
            value => anyhow::bail!("invalid `related_mode`: `{value}`, expected all or essential"),
        }
    }
}

fn strip_comment(line: &str) -> &str {
    line.split_once('#').map_or(line, |(before, _)| before).trim()
}

fn expand_tilde(path: &str, mpv: &mut Handle) -> Result<PathBuf> {
    if path == "~" {
        return mpv.expand_path("~/");
    }

    if let Some(rest) = path.strip_prefix("~/") {
        return Ok(mpv.expand_path("~/")?.join(rest));
    }

    Ok(PathBuf::from(path))
}

fn parse_key_value_conf(input: &str) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    for (line_index, raw_line) in input.lines().enumerate() {
        let line = strip_comment(raw_line);

        if line.is_empty() {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            anyhow::bail!(
                "invalid config line {}: `{}`; expected `key=value`",
                line_index + 1,
                line
            );
        };

        map.insert(key.trim().to_owned(), value.trim().to_owned());
    }

    Ok(map)
}

impl Config {
    pub fn load(mpv: &mut Handle) -> Result<Self> {
        let path = mpv.expand_path(CONFIG_PATH)?;
        let input = fs::read_to_string(&path).unwrap_or_default();

        Self::parse(&input, mpv)
    }

    fn parse(input: &str, mpv: &mut Handle) -> Result<Self> {
        let map = parse_key_value_conf(input)?;
        let script_opts = mpv.get_script_opts()?;

        let get_opt = |key: &str| -> Option<&str> {
            script_opts
                .get(&format!("kodik-{key}"))
                .and_then(|node| match node {
                    Node::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .or_else(|| map.get(key).map(String::as_str))
        };

        let quality = get_opt("quality").map(str::parse).transpose()?.unwrap_or(Quality::P720);
        let cookies = get_opt("cookies").map(|path| expand_tilde(path, mpv)).transpose()?;
        let translation_title = get_opt("translation_title").map(ToOwned::to_owned);
        let translation_type = get_opt("translation_type").map(str::parse).transpose()?;
        let log_level = get_opt("log_level").map_or(Ok(LevelFilter::Error), str::parse)?;
        let related_mode = get_opt("related_mode").map_or(Ok(RelatedMode::None), str::parse)?;

        Ok(Self {
            quality,
            cookies,
            translation_title,
            translation_type,
            related_mode,
            log_level,
        })
    }

    pub const fn quality(&self) -> Quality {
        self.quality
    }

    pub const fn cookies(&self) -> Option<&PathBuf> {
        self.cookies.as_ref()
    }

    pub fn translation_title(&self) -> Option<&str> {
        self.translation_title.as_deref()
    }

    pub const fn translation_type(&self) -> Option<TranslationType> {
        self.translation_type
    }

    pub const fn related_mode(&self) -> RelatedMode {
        self.related_mode
    }

    pub fn load_cookies(&self) -> Result<Jar> {
        let jar = Jar::default();

        let Some(cookies) = &self.cookies else {
            return Ok(jar);
        };

        let file = File::open(cookies)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        while reader.read_line(&mut line)? > 0 {
            let trimmed = line.trim();

            if trimmed.starts_with('#') || trimmed.is_empty() {
                line.clear();
                continue;
            }

            let mut parts = trimmed.splitn(7, '\t');

            let domain = parts.next().context("malformed cookie: missing domain")?;
            let key = parts.nth(4).context("malformed cookie: missing name")?;
            let value = parts.next().context("malformed cookie: missing value")?;

            let mut cookie = String::with_capacity(key.len() + value.len() + domain.len() + 10);
            cookie.push_str(key);
            cookie.push('=');
            cookie.push_str(value);
            cookie.push_str("; Domain=");
            cookie.push_str(domain);

            let domain = domain.trim_start_matches('.');
            let mut url_str = String::with_capacity(8 + domain.len());
            url_str.push_str("https://");
            url_str.push_str(domain);
            let url = Url::parse(&url_str)?;

            jar.add_cookie_str(&cookie, &url);

            line.clear();
        }

        Ok(jar)
    }

    pub const fn log_level(&self) -> LevelFilter {
        self.log_level
    }

    pub fn set_translation_title(&mut self, translation_title: Option<String>) {
        self.translation_title = translation_title;
    }

    pub const fn set_quality(&mut self, quality: Quality) {
        self.quality = quality;
    }
}
