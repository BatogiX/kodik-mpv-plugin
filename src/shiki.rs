use anyhow::{Context as _, Result};
use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use kodik_shiki::{Related, UserRate};
use mpv_client::Node;
use serde::{Deserialize, Serialize};

use crate::{mpv_ext::MpvResultExt as _, state::PluginState};

struct AnimeMetaData {
    id: usize,
    name: String,
    episodes: usize,
    user_rate: Option<UserRate>,
}

impl AnimeMetaData {
    const fn new(id: usize, name: String, episodes: usize, user_rate: Option<UserRate>) -> Self {
        Self {
            id,
            name,
            episodes,
            user_rate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimePayload {
    pub anime_id: usize,
    pub episode: usize,
}

impl AnimePayload {
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

#[derive(Debug, Clone)]
pub struct MpvFileOptions {
    pub title: Option<String>,
    pub payload: Option<AnimePayload>,
}

impl MpvFileOptions {
    pub fn to_mpv_options_string(&self) -> Result<String> {
        let mut options = Vec::new();

        if let Some(title) = &self.title {
            options.push(format!("force-media-title={}", escape_mpv_option_value(title)));
        }

        if let Some(payload) = &self.payload {
            let encoded = payload.encode()?;

            options.push(format!(
                "script-opts-append=kodik-payload={}",
                escape_mpv_option_value(&encoded)
            ));
        }

        Ok(options.join(","))
    }
}

fn escape_mpv_option_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace(',', "\\,")
}

pub fn expand_by_related(state: &mut PluginState, url: &str, host: &str, index: usize) -> Result<()> {
    let animes = state.runtime().block_on(async {
        let shiki_api_animes = kodik_shiki::fetch_shiki_api_animes(state.client(), url).await?;

        let Some(ref franchise) = shiki_api_animes.franchise else {
            return Ok(vec![AnimeMetaData::new(
                shiki_api_animes.id,
                shiki_api_animes.name,
                shiki_api_animes.episodes,
                shiki_api_animes.user_rate,
            )]);
        };

        let not_anime_ids = kodik_shiki::fetch_not_anime_ids(state.client(), franchise)
            .await?
            .unwrap_or(&[]);

        let related = {
            let mut related = Related::fetch_by_franchise(state.client(), franchise, host).await?;
            let _ = related.filter_by_not_anime_ids(not_anime_ids)?.sort_by_chrono();
            related
        };

        Ok::<Vec<AnimeMetaData>, anyhow::Error>(
            related
                .animes
                .into_iter()
                .map(|anime| {
                    AnimeMetaData::new(
                        anime.id.parse().unwrap(),
                        anime.name,
                        anime.episodes,
                        Some(UserRate::new(anime.user_rate.map_or(0, |ur| ur.episodes))),
                    )
                })
                .collect(),
        )
    })?;

    let mut insert_index = index + 1;
    for anime in animes {
        for episode in 1..=anime.episodes {
            let media_title = format!("{} - Episode {}", anime.name, episode);

            let payload = AnimePayload {
                anime_id: anime.id,
                episode,
            };

            let file_options = MpvFileOptions {
                title: Some(media_title.clone()),
                payload: Some(payload),
            };

            let options = file_options.to_mpv_options_string()?;

            state
                .mpv_mut()
                .command([
                    "loadfile",
                    media_title.as_str(),
                    // format!("https://{host}/animes/{}", anime.id).as_str(),
                    "insert-at",
                    insert_index.to_string().as_str(),
                    options.as_str(),
                ])
                .mpv_context("failed to append file")?;

            insert_index += 1;
        }
    }

    state
        .mpv_mut()
        .command(["playlist-remove", index.to_string().as_str()])
        .mpv_context("failed to remove original playlist entry")?;

    Ok(())
}

pub fn on_load(state: &mut PluginState) -> Result<()> {
    let node: Node = state
        .mpv_mut()
        .get_property("options/script-opts")
        .mpv_context("failed to get script-opts")?;

    let Node::Map(mut script_opts) = node else {
        return Ok(());
    };

    let Some(kodik_playload) = script_opts.remove("kodik-payload") else {
        return Ok(());
    };

    let Node::String(kodik_playload) = kodik_playload else {
        return Ok(());
    };

    let anime_payload = AnimePayload::decode(&kodik_playload)?;

    let episode = state.runtime().block_on(async {
        let kodik_videos = kodik_shiki::fetch_kodik_videos(state.client(), anime_payload.anime_id).await?;

        let search_result = kodik_videos.find_search_result(None, None)?.clone();

        let episode = match search_result.seasons {
            Some(mut seasons) => seasons
                .pop_last()
                .unwrap()
                .1
                .episodes
                .remove(&anime_payload.episode)
                .unwrap(),
            None => search_result.link,
        };

        let mut kodik_response = kodik_parser::parse(state.client(), format!("https:{episode}").as_str()).await?;
        let episode = kodik_response.links.quality_720.remove(0).src;

        Ok::<String, anyhow::Error>(episode)
    })?;

    state
        .mpv_mut()
        .set_property("stream-open-filename", episode)
        .mpv_context("failed to spoof stream-open-filename")?;

    Ok(())
}
