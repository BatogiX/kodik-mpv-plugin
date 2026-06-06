use anyhow::Result;
use kodik_parser::TranslationType;
use kodik_utils::{Jar, JarExt};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
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
}

impl Config {
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
        Ok(match &self.cookies {
            Some(path) => Jar::load_netscape(path)?,
            None => Jar::default(),
        })
    }

    pub fn set_translation_title(&mut self, translation_title: Option<String>) {
        self.translation_title = translation_title;
    }

    pub const fn set_quality(&mut self, quality: Quality) {
        self.quality = quality;
    }

    pub fn set_cookies(&mut self, cookies: Option<PathBuf>) {
        self.cookies = cookies;
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum Quality {
    #[default]
    #[serde(alias = "720")]
    P720,

    #[serde(alias = "480")]
    P480,

    #[serde(alias = "360")]
    P360,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RelatedMode {
    All,
    Essential,
    #[default]
    None,
}
