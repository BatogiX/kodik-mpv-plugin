use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
};

use anyhow::Result;
use kodik_parser::KODIK_STATE;
use mpv_client::Handle;
use serde::{Deserialize, Serialize};

use crate::mpv_ext::MpvExt;

const CACHE_PATH: &str = "~~cache/kodik.json";

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Cache {
    endpoint: String,
    shift: u8,
    #[serde(skip)]
    path: PathBuf,
}

impl Cache {
    pub fn load(mp: &Handle) -> Result<Self> {
        let path = mp.expand_path(CACHE_PATH)?;

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            File::create(&path)?;
        }

        let content = fs::read_to_string(&path)?;
        let mut cache = serde_json::from_str::<Self>(&content).unwrap_or_default();
        cache.path = path;

        KODIK_STATE.set_shift(cache.shift);
        KODIK_STATE.set_endpoint(cache.endpoint.clone());

        anyhow::Ok(cache)
    }

    pub fn update_and_save(&mut self) -> Result<()> {
        let current_shift = KODIK_STATE.shift();
        let current_endpoint = KODIK_STATE.endpoint();

        if self.shift == current_shift && self.endpoint == *current_endpoint {
            return Ok(());
        }

        self.shift = current_shift;
        self.endpoint = current_endpoint.to_string();

        let file = OpenOptions::new().write(true).truncate(true).open(&self.path)?;
        serde_json::to_writer_pretty(file, self)?;

        Ok(())
    }
}
