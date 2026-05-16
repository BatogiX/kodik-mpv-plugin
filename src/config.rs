use anyhow::{Context as _, Result};
use kodik_shiki::TranslationType;
use log::LevelFilter;
use reqwest::{Url, cookie::Jar};
use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{BufRead as _, BufReader},
    path::PathBuf,
    str::FromStr,
};

#[derive(Debug, Clone, Copy)]
pub enum Quality {
    P720,
    P480,
    P360,
}

#[derive(Debug, Clone, Copy)]
pub enum RelatedMode {
    All,
    Essential,
}

#[derive(Debug)]
pub struct Config {
    /// Specify video quality [default: 720] [possible values: 360, 480, 720]
    quality: Quality,

    /// Netscape formatted file to read cookies from
    cookies: PathBuf,

    /// Specify translation title
    translation_title: String,

    /// Specify translation type [possible values: voice, subtitles]
    translation_type: TranslationType,

    /// Expand a media database URL into all related URLs [possible values: all, essential]
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
    pub fn load() -> Result<Self> {
        let path = default_config_path()?;

        eprintln!("loading config from: {}", path.display());

        Self::from_conf_file(path)
    }

    fn from_conf_str(input: &str) -> Result<Self> {
        let map = parse_key_value_conf(input)?;

        let quality = match map.get("quality") {
            Some(value) => value.parse()?,
            None => Quality::P720,
        };

        let cookies = expand_tilde(required(&map, "cookies")?);
        let translation_title = required(&map, "translation_title")?.to_string();
        let translation_type = required(&map, "translation_type")?.parse::<TranslationType>()?;
        let log_level = required(&map, "log_level")?.parse::<LevelFilter>()?;
        let related_mode = required(&map, "related_mode")?
            .parse()
            .context("failed to parse `related_mode`")?;

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
        let input = std::fs::read_to_string(path)?;
        Self::from_conf_str(&input)
    }

    pub const fn quality(&self) -> Quality {
        self.quality
    }

    pub const fn cookies(&self) -> &PathBuf {
        &self.cookies
    }

    pub fn translation_title(&self) -> &str {
        &self.translation_title
    }

    pub const fn translation_type(&self) -> TranslationType {
        self.translation_type
    }

    pub const fn related_mode(&self) -> RelatedMode {
        self.related_mode
    }

    pub fn load_cookies(&self) -> Result<Jar> {
        let jar = Jar::default();

        if !self.cookies.as_os_str().is_empty() {
            let file = File::open(self.cookies())?;
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
        }

        Ok(jar)
    }

    pub const fn log_level(&self) -> LevelFilter {
        self.log_level
    }
}

fn default_config_path() -> Result<PathBuf> {
    Ok(mpv_config_dir()?.join("script-opts").join("kodik.conf"))
}

fn mpv_config_dir() -> Result<PathBuf> {
    if let Some(mpv_home) = env::var_os("MPV_HOME") {
        return Ok(PathBuf::from(mpv_home));
    }

    #[cfg(windows)]
    {
        let appdata = env::var_os("APPDATA")
            .map(PathBuf::from)
            .context("APPDATA environment variable is not set")?;

        Ok(appdata.join("mpv"))
    }

    #[cfg(not(windows))]
    {
        if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(xdg_config_home).join("mpv"));
        }

        let home = home_dir().context("HOME environment variable is not set")?;

        Ok(home.join(".config").join("mpv"))
    }
}
