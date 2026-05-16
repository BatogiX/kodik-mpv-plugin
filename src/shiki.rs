use anyhow::{Context as _, Result};
use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use kodik_shiki::{AnimeStatus, Related, UserRate, UserRateStatus};
use kodik_utils::{GET, PATCH as _, POST};
use mpv_client::Node;
use serde::{Deserialize, Serialize};

use crate::{config::Quality, mpv_ext::MpvResultExt as _, state::PluginState};

struct AnimeMetaData {
    id: usize,
    name: String,
    status: AnimeStatus,
    episodes: usize,
    episodes_aired: usize,
    user_rate: Option<UserRate>,
}

impl AnimeMetaData {
    const fn new(
        id: usize,
        name: String,
        status: AnimeStatus,
        episodes: usize,
        episodes_aired: usize,
        user_rate: Option<UserRate>,
    ) -> Self {
        Self {
            id,
            name,
            status,
            episodes,
            episodes_aired,
            user_rate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimePayload {
    pub anime_id: usize,
    pub episode: usize,
    pub episodes: usize,
    pub episodes_aired: usize,
    pub host: String,
    pub user_rate: Option<UserRate>,
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
                shiki_api_animes.status,
                shiki_api_animes.episodes,
                shiki_api_animes.episodes_aired,
                shiki_api_animes.user_rate,
            )]);
        };

        let not_anime_ids = kodik_shiki::fetch_not_anime_ids(state.client(), franchise)
            .await?
            .unwrap_or(&[]);

        let related = {
            let mut related = Related::fetch_by_franchise(state.client(), franchise, host, not_anime_ids).await?;
            related.sort_by_chrono();
            related
        };

        Ok::<Vec<AnimeMetaData>, anyhow::Error>(
            related
                .animes
                .into_iter()
                .map(|anime| {
                    AnimeMetaData::new(
                        anime.id,
                        anime.name,
                        anime.status,
                        anime.episodes,
                        anime.episodes_aired,
                        anime
                            .user_rate
                            .map(|ur| UserRate::new(ur.id, ur.status, ur.episodes, ur.rewatches)),
                    )
                })
                .collect(),
        )
    })?;

    let mut insert_index = index + 1;
    for anime in animes {
        let episodes = if anime.status == AnimeStatus::Ongoing {
            anime.episodes_aired
        } else {
            anime.episodes
        };

        for episode in 1..=episodes {
            // if let Some(ref user_rate) = anime.user_rate
            //     && user_rate.episodes >= episode
            // {
            //     continue;
            // }

            let media_title = if anime.episodes == 1 {
                format!("{} - Movie", anime.name)
            } else {
                format!("{} - Episode {}", anime.name, episode)
            };

            let payload = AnimePayload {
                anime_id: anime.id,
                episode,
                user_rate: anime.user_rate,
                episodes: anime.episodes,
                episodes_aired: anime.episodes_aired,
                host: host.to_owned(),
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

    let links = state.runtime().block_on(async {
        let kodik_videos = kodik_shiki::fetch_kodik_videos(state.client(), anime_payload.anime_id).await?;

        let search_result = kodik_videos.find_search_result(
            Some(state.config().translation_title()),
            Some(&state.config().translation_type()),
        )?;

        let Some(episode) = search_result
            .seasons
            .as_ref()
            .map_or(Some(&search_result.link), |seasons| {
                seasons.iter().last().unwrap().1.episodes.get(&anime_payload.episode)
            })
        else {
            anyhow::bail!("episode not found");
        };

        let mut links = kodik_parser::parse(state.client(), format!("https:{episode}").as_str()).await?;

        Ok::<[String; 3], anyhow::Error>([
            links.quality_720.remove(0).src,
            links.quality_480.remove(0).src,
            links.quality_360.remove(0).src,
        ])
    });

    if let Ok(mut links) = links {
        match state.config().quality() {
            Quality::P720 => {}
            Quality::P480 => links.swap(1, 0),
            Quality::P360 => links.swap(2, 0),
        }

        for link in links {
            let text = state
                .runtime()
                .block_on(async { state.client().fetch_as_text(&link).await })?;

            if text.is_empty() {
                continue;
            }

            state
                .mpv_mut()
                .set_property("stream-open-filename", link)
                .mpv_context("failed to spoof stream-open-filename")?;

            break;
        }
    }

    Ok(())
}

pub fn mark_as_watched(state: &mut PluginState) -> Result<()> {
    let node: Node = state
        .mpv_mut()
        .get_property("options/script-opts")
        .mpv_context("failed to get script-opts")?;

    state
        .mpv_mut()
        .command(["playlist-next"])
        .mpv_context("failed to send command `playlist-next`")?;

    let Node::Map(mut script_opts) = node else {
        anyhow::bail!("`script_opts` is not a HashMap")
    };

    let Some(kodik_playload) = script_opts.remove("kodik-payload") else {
        anyhow::bail!("there is no `kodik-payload` in `script_opts`")
    };

    let Node::String(kodik_playload) = kodik_playload else {
        anyhow::bail!("`kodik_playload` is not a String")
    };

    let anime_payload = AnimePayload::decode(&kodik_playload)?;
    let user_id = {
        let whoami: ShikiApiUsersWhoami = state.runtime().block_on(async {
            state
                .client()
                .fetch_as_json(&format!("https://{}/api/users/whoami", anime_payload.host))
                .await
        })?;

        whoami.id
    };

    println!("playload is: {anime_payload:#?}");

    if let Some(user_rate) = anime_payload.user_rate {
        let (user_rate_rewatches, user_rate_status) = if user_rate.status == UserRateStatus::Completed {
            (user_rate.rewatches + 1, UserRateStatus::Rewatching)
        } else {
            (user_rate.rewatches, UserRateStatus::Watching)
        };

        let shiki_api_user_rates = ShikiApiUserRates::new(
            anime_payload.episode,
            user_rate_rewatches,
            user_rate_status,
            anime_payload.anime_id,
            UserRatesTargetType::Anime,
            user_id,
        );

        state.runtime().block_on(async {
            let _: UserRate = state
                .client()
                .patch_json_as_json(
                    &format!("https://{}/api/v2/user_rates/{}", anime_payload.host, user_rate.id),
                    &shiki_api_user_rates,
                )
                .await
                .unwrap();
        });
    } else {
        let user_rate_status = if anime_payload.episodes == 1 || anime_payload.episodes == anime_payload.episode {
            UserRateStatus::Completed
        } else {
            UserRateStatus::Watching
        };

        let shiki_api_user_rates = ShikiApiUserRates::new(
            anime_payload.episode,
            0,
            user_rate_status,
            anime_payload.anime_id,
            UserRatesTargetType::Anime,
            user_id,
        );

        state.runtime().block_on(async {
            let _: UserRate = state
                .client()
                .post_json_as_json(
                    &format!("https://{}/api/v2/user_rates/", anime_payload.host),
                    &shiki_api_user_rates,
                )
                .await
                .unwrap();
        });
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct ShikiApiUserRates {
    user_rate: ShikiApiUserRatesUserRate,
}

impl ShikiApiUserRates {
    const fn new(
        episodes: usize,
        rewatches: usize,
        status: UserRateStatus,
        target_id: usize,
        target_type: UserRatesTargetType,
        user_id: usize,
    ) -> Self {
        Self {
            user_rate: ShikiApiUserRatesUserRate::new(episodes, rewatches, status, target_id, target_type, user_id),
        }
    }
}

#[derive(Debug, Serialize)]
struct ShikiApiUserRatesUserRate {
    pub episodes: usize,
    pub rewatches: usize,
    pub status: UserRateStatus,
    pub target_id: usize,
    pub target_type: UserRatesTargetType,
    pub user_id: usize,
}

impl ShikiApiUserRatesUserRate {
    const fn new(
        episodes: usize,
        rewatches: usize,
        status: UserRateStatus,
        target_id: usize,
        target_type: UserRatesTargetType,
        user_id: usize,
    ) -> Self {
        Self {
            episodes,
            rewatches,
            status,
            target_id,
            target_type,
            user_id,
        }
    }
}

#[derive(Debug, Serialize)]
enum UserRatesTargetType {
    Anime,
    Manga,
    VisualNovel,
}

#[derive(Debug, Deserialize)]
struct ShikiApiUsersWhoami {
    id: usize,
}
