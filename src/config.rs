use anyhow::{Context as _, Result};
use kodik_shiki::TranslationType;
use log::LevelFilter;
use mpv_client::{Handle, Node};
use reqwest::{Url, cookie::Jar};
use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::{BufRead as _, BufReader},
    path::PathBuf,
    str::FromStr,
};

use crate::mpv_ext::MpvResultExt;

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

fn expand_tilde(path: &str) -> PathBuf {
    if path == "~"
        && let Some(home) = home_dir()
    {
        return home;
    }

    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }

    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
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

        map.insert(key.trim().to_string(), value.trim().to_string());
    }

    Ok(map)
}

fn required<'a>(map: &'a HashMap<String, String>, key: &str) -> Result<&'a str> {
    map.get(key)
        .map(String::as_str)
        .with_context(|| format!("missing required config field `{key}`"))
}

impl Config {
    pub fn load(mpv: &mut Handle) -> Result<Self> {
        let node = mpv
            .command_ret(["expand-path", "~~/"])
            .mpv_context("failed to `expand-path \"~~/\"`")?;

        let Node::String(path) = node else {
            anyhow::bail!("`expand-path \"~~/\"` returned non-string value");
        };

        Self::from_conf_file(PathBuf::from(path).join("script-opts").join("kodik.conf"))
    }

    fn from_conf_str(input: &str) -> Result<Self> {
        let map = parse_key_value_conf(input)?;

        let quality = match map.get("quality") {
            Some(value) => value.parse()?,
            None => Quality::P720,
        };

        let cookies = required(&map, "cookies").ok().map(expand_tilde);
        let translation_title = required(&map, "translation_title")
            .ok()
            .map(std::borrow::ToOwned::to_owned);
        let translation_type = required(&map, "translation_type")
            .ok()
            .map(str::parse::<TranslationType>)
            .transpose()?;
        let log_level = required(&map, "log_level").map_or(Ok(LevelFilter::Error), str::parse::<LevelFilter>)?;
        let related_mode = required(&map, "related_mode").map_or(Ok(RelatedMode::None), str::parse)?;

        Ok(Self {
            quality,
            cookies,
            translation_title,
            translation_type,
            related_mode,
            log_level,
        })
    }

    fn from_conf_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        fs::read_to_string(path).map_or_else(|_| Self::from_conf_str(""), |input| Self::from_conf_str(&input))
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
}
