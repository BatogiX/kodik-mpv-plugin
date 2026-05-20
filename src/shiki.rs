use std::{fmt::Display, str::FromStr, time::Duration};

use crate::hooks::{MpvFileOptions, Payload};
use anyhow::{Context as _, Result};
use kodik_shiki::{AnimeStatus, Related, UserRate, UserRateStatus};
use kodik_utils::{GET, PATCH as _, POST};
use reqwest::{Url, cookie::CookieStore};
use serde::{Deserialize, Serialize};

use crate::{
    config::{Quality, RelatedMode},
    mpv_ext::MpvResultExt as _,
    state::PluginState,
};

struct Anime {
    id: usize,
    name: String,
    status: AnimeStatus,
    episodes: usize,
    episodes_aired: usize,
    user_rate: Option<UserRate>,
}

impl Anime {
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
pub struct ShikiPayload {
    pub anime_id: usize,
    pub episode: usize,
    pub episodes: usize,
    pub host: String,
    pub user_rate: Option<UserRate>,
    pub user_id: Option<usize>,
}

#[derive(Debug, Serialize)]
enum UserRatesTargetType {
    Anime,
    // Manga,
    // VisualNovel,
}

#[derive(Debug, Deserialize)]
struct ShikiApiUsersWhoami {
    id: usize,
}

pub fn expand(state: &mut PluginState, url: &str, host: &str) -> Result<()> {
    let animes = state.runtime().block_on(async {
        let shiki_api_animes = kodik_shiki::fetch_shiki_api_animes(state.client(), url).await?;

        let Some(franchise) = shiki_api_animes
            .franchise
            .as_ref()
            .filter(|_| state.config().related_mode() != RelatedMode::None)
        else {
            return Ok(vec![Anime::new(
                shiki_api_animes.id,
                shiki_api_animes.name,
                shiki_api_animes.status,
                shiki_api_animes.episodes,
                shiki_api_animes.episodes_aired,
                shiki_api_animes.user_rate,
            )]);
        };

        let mut related = if state.config().related_mode() == RelatedMode::Essential {
            let not_anime_ids = kodik_shiki::fetch_not_anime_ids(state.client(), franchise)
                .await?
                .unwrap_or(&[]);

            Related::fetch_by_franchise(state.client(), franchise, host, not_anime_ids).await?
        } else {
            Related::fetch_by_franchise(state.client(), franchise, host, &[]).await?
        };
        related.sort_by_chrono();

        Ok::<Vec<Anime>, anyhow::Error>(
            related
                .animes
                .into_iter()
                .map(|anime| {
                    Anime::new(
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

    let user_id = state.runtime().block_on(async {
        state
            .client()
            .fetch_as_json::<ShikiApiUsersWhoami>(&format!("https://{host}/api/users/whoami"))
            .await
            .ok()
            .map(|whoami| whoami.id)
    });

    let current_index: i64 = state
        .mpv_mut()
        .get_property("playlist-pos")
        .mpv_context("failed to get current playlist position")?;

    let mut insert_index = current_index + 1;
    let mut target_index: Option<i64> = None;
    for anime in animes {
        let episodes = if anime.status == AnimeStatus::Ongoing {
            anime.episodes_aired
        } else {
            anime.episodes
        };

        for episode in 1..=episodes {
            let mut media_title = if anime.episodes == 1 {
                format!("{} - Movie", anime.name)
            } else {
                format!("{} - Episode {}", anime.name, episode)
            };

            if let Some(user_rate) = anime.user_rate {
                if user_rate.episodes >= episode {
                    media_title.push_str(" ✓");
                } else {
                    match user_rate.status {
                        UserRateStatus::Watching => media_title.push_str(" ▶"),
                        UserRateStatus::Rewatching => media_title.push_str(" ↻"),
                        UserRateStatus::Completed => media_title.push_str(" ✓"),
                        _ => {}
                    }
                }
            }

            if target_index.is_none() && !media_title.ends_with('✓') {
                target_index = Some(insert_index - 1);
            }

            let shiki_payload = ShikiPayload {
                anime_id: anime.id,
                episode,
                user_rate: anime.user_rate,
                episodes: anime.episodes,
                host: host.to_owned(),
                user_id,
            };

            let file_options = MpvFileOptions {
                payload: Payload::Shiki(shiki_payload),
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
        .command(["playlist-remove", &current_index.to_string()])
        .mpv_context("failed to remove original playlist entry")?;

    if let Some(target_index) = target_index {
        state
            .mpv_mut()
            .command(["set", "playlist-pos", &target_index.to_string()])
            .mpv_context("failed to set current playlist position")?;
    }

    Ok(())
}

pub fn on_load(state: &mut PluginState, payload: &ShikiPayload) -> Result<()> {
    let key = format!("{}/animes/{}", payload.host, payload.anime_id);

    if !state.kodik_videos_mut().contains_key(&key) {
        let videos = state
            .runtime()
            .block_on(kodik_shiki::fetch_kodik_videos(state.client(), payload.anime_id))?;

        state.kodik_videos_mut().insert(key.clone(), videos);
    }

    let kodik_videos = state
        .kodik_videos()
        .get(&key)
        .context("kodik videos should exist after insert")?;

    let result = match kodik_videos.find_result(state.config().translation_title(), state.config().translation_type()) {
        Ok(result) => result,
        Err(err) => {
            state
                .mpv_mut()
                .command(["playlist-next", "weak"])
                .mpv_context("failed to `playlist-next weak`")?;

            return Err(err.into());
        }
    };

    let Some(episode) = result.seasons.as_ref().map_or(Some(&result.link), |seasons| {
        seasons
            .iter()
            .last()
            .and_then(|(_, season)| season.episodes.get(&payload.episode))
    }) else {
        state
            .mpv_mut()
            .command(["playlist-next", "weak"])
            .mpv_context("failed to `playlist-next weak`")?;

        anyhow::bail!("episode not found");
    };

    let links = state.runtime().block_on(async {
        let links = kodik_parser::parse(state.client(), format!("https:{episode}").as_str()).await?;
        Ok::<[String; 3], anyhow::Error>([links.p720, links.p480, links.p360])
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
                .mpv_context("failed to substitute stream-open-filename")?;

            break;
        }
    }

    Ok(())
}

fn mark_as_watched_osd_text(
    episode: impl Display,
    episodes: impl Display,
    status: UserRateStatus,
    rewatches: impl Display,
    completed_rewatch: bool,
) -> String {
    match status {
        UserRateStatus::Completed if completed_rewatch => {
            format!("✓ Rewatch completed: {episode}/{episodes} — rewatch #{rewatches}")
        }
        UserRateStatus::Completed => format!("✓ Marked as completed: {episode}/{episodes}"),
        UserRateStatus::Rewatching => format!("↻ Marked as watched: {episode}/{episodes} — rewatch #{rewatches}"),
        UserRateStatus::Watching => format!("▶ Marked as watched: {episode}/{episodes}"),
        _ => String::new(),
    }
}

pub fn mark_as_watched(state: &mut PluginState, shiki_payload: ShikiPayload) -> Result<()> {
    let url = Url::from_str(&format!("https://{}", shiki_payload.host))?;

    let has_kawai_session = state
        .jar()
        .cookies(&url)
        .map(|cookies| cookies.to_str().map(|s| s.contains("_kawai_session")))
        .transpose()?
        .unwrap_or(false);

    if state.config().cookies().is_none() || !has_kawai_session {
        anyhow::bail!("there is no cookies for `{url}`");
    }

    let Some(user_id) = shiki_payload.user_id else {
        anyhow::bail!("there is no `user_id` in payload")
    };

    let mut mpv_client = state
        .mpv_mut()
        .create_client("kodik-mpv-client")
        .mpv_context("failed to create mpv client")?;

    let client = state.client().clone();
    state.runtime().spawn(async move {
        let result = async {
            let is_last_episode = shiki_payload.episode == shiki_payload.episodes;
            let osd_text = if let Some(user_rate) = shiki_payload.user_rate.as_ref() {
                let (user_rate_rewatches, user_rate_status, completed_rewatch) = if is_last_episode {
                    let completed_rewatch =
                        user_rate.status == UserRateStatus::Rewatching || user_rate.status == UserRateStatus::Completed;

                    let rewatches = if completed_rewatch {
                        user_rate.rewatches + 1
                    } else {
                        user_rate.rewatches
                    };

                    (rewatches, UserRateStatus::Completed, completed_rewatch)
                } else if user_rate.status == UserRateStatus::Completed {
                    (user_rate.rewatches, UserRateStatus::Rewatching, false)
                } else {
                    (user_rate.rewatches, UserRateStatus::Watching, false)
                };

                let shiki_api_user_rates = ShikiApiUserRates::new(
                    shiki_payload.episode,
                    user_rate_rewatches,
                    user_rate_status,
                    shiki_payload.anime_id,
                    UserRatesTargetType::Anime,
                    user_id,
                );

                let _ = client
                    .patch_json_as_text(
                        &format!("https://{}/api/v2/user_rates/{}", shiki_payload.host, user_rate.id),
                        &shiki_api_user_rates,
                    )
                    .await?;

                mark_as_watched_osd_text(
                    shiki_payload.episode,
                    shiki_payload.episodes,
                    user_rate_status,
                    user_rate_rewatches,
                    completed_rewatch,
                )
            } else {
                let user_rate_status = if is_last_episode {
                    UserRateStatus::Completed
                } else {
                    UserRateStatus::Watching
                };

                let shiki_api_user_rates = ShikiApiUserRates::new(
                    shiki_payload.episode,
                    0,
                    user_rate_status,
                    shiki_payload.anime_id,
                    UserRatesTargetType::Anime,
                    user_id,
                );

                let _ = client
                    .post_json_as_text(
                        &format!("https://{}/api/v2/user_rates", shiki_payload.host),
                        &shiki_api_user_rates,
                    )
                    .await?;

                mark_as_watched_osd_text(
                    shiki_payload.episode,
                    shiki_payload.episodes,
                    user_rate_status,
                    0,
                    false,
                )
            };

            let _ = mpv_client::osd!(mpv_client, Duration::from_secs(8), "{osd_text}");

            anyhow::Ok(())
        }
        .await;

        if let Err(err) = result {
            log::error!("failed to mark episode as watched: {err:?}");
        }
    });

    let _ = state.mpv_mut().command(["playlist-next"]);

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
