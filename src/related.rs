use crate::{MpvResultExt as _, PluginState, models::PlaylistEntry};
use anyhow::{Context as _, Result};
use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use kodik_shiki::{Related, UserRate};
use mpv_client::Node;
use reqwest::Url;
use serde::{Deserialize, Serialize};

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

pub fn expand_by_related(state: &mut PluginState) -> Result<()> {
    let Ok(playlist_count) = state.mpv_mut().get_property::<i64>("playlist-count") else {
        return Ok(());
    };

    if playlist_count == 0 {
        return Ok(());
    }

    let Ok(node) = state.mpv_mut().get_property("playlist") else {
        anyhow::bail!("there is no playlist in property");
    };

    let Node::Array(entries) = node else {
        anyhow::bail!("playlist is not array");
    };

    let mut playlist = Vec::with_capacity(entries.len());
    for entry in entries {
        let Node::Map(mut entry) = entry else {
            continue;
        };

        let Some(Node::String(filename)) = entry.remove("filename") else {
            continue;
        };

        let Some(Node::Int(id)) = entry.remove("id") else {
            continue;
        };

        playlist.push(PlaylistEntry::new(filename, id));
    }

    for (index, (_id, filename)) in playlist
        .iter()
        .map(|playlist_entry| (playlist_entry.id(), playlist_entry.filename()))
        .enumerate()
    {
        println!("filename is: {filename}");

        let Ok(url) = Url::parse(filename) else {
            continue;
        };

        let Some(host) = url.host_str() else {
            continue;
        };

        let Some(host_name) = host.rsplit_once('.').map(|(lp, _rp)| lp) else {
            continue;
        };

        if host_name == "shikimori" {
            let animes = state.runtime().block_on(async {
                let shiki_api_animes = kodik_shiki::fetch_shiki_api_animes(state.client(), url.as_str()).await?;

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
        }
    }

    // let node: Node = state
    //     .mpv_mut()
    //     .get_property("playlist")
    //     .mpv_context("failed to get playlist")?;

    // let Node::Array(entries) = node else {
    //     anyhow::bail!("playlist is not array");
    // };

    // let mut playlist = Vec::with_capacity(entries.len());
    // for entry in entries {
    //     let Node::Map(mut entry) = entry else {
    //         continue;
    //     };

    //     let Some(Node::String(filename)) = entry.remove("filename") else {
    //         continue;
    //     };

    //     let Some(Node::Int(id)) = entry.remove("id") else {
    //         continue;
    //     };

    //     playlist.push(PlaylistEntry::new(filename, id));
    // }

    // println!("{playlist:#?}");

    Ok(())
}
