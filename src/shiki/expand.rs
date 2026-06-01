use std::collections::hash_map::Entry;
use std::str::FromStr as _;

use crate::config::Config;
use crate::mpv_ext::MpvExt;
use crate::shiki::{COMPLETED_CHAR, DROPPED_CHAR, ONHOLD_CHAR, PLANNED_CHAR, REWATCHING_CHAR, WATCHING_CHAR};
use crate::{
    events::{MetaData, Payload},
    shiki::ShikiMetaData,
};
use anyhow::Result;
use kodik_shiki::{AnimeStatus, Related, ShikiApiAnimes, ShikiApiUsersWhoami, UserRateStatus};
use mpv_client::Handle;
use reqwest::cookie::CookieStore as _;
use reqwest::{Client, Url};

use crate::{config::RelatedMode, state::PluginState};

async fn fetch_user_id(client: &Client, host: &str) -> Result<Option<usize>> {
    let user_id = ShikiApiUsersWhoami::fetch(client, host).await?.id;

    anyhow::Ok(user_id)
}

async fn fetch_animes(client: &Client, config: &Config, url: &str, host: &str) -> Result<Vec<ShikiMetaData>> {
    let shiki_api_animes = ShikiApiAnimes::fetch(client, url).await?;

    let Some(franchise) = shiki_api_animes
        .franchise
        .as_ref()
        .filter(|_| config.related_mode() != RelatedMode::None)
    else {
        return Ok(vec![ShikiMetaData::new(
            shiki_api_animes.id,
            shiki_api_animes.name,
            shiki_api_animes.episodes,
            shiki_api_animes.episodes_aired,
            shiki_api_animes.status,
            shiki_api_animes.kind,
            shiki_api_animes.user_rate,
            host.to_owned(),
            None,
        )]);
    };

    let mut related = if config.related_mode() == RelatedMode::Essential {
        let not_anime_ids = kodik_shiki::fetch_not_anime_ids(client, franchise)
            .await?
            .unwrap_or(&[]);

        Related::fetch_by_franchise(client, franchise, host, not_anime_ids).await?
    } else {
        Related::fetch_by_franchise(client, franchise, host, &[]).await?
    };

    related.sort_by_chrono();

    Ok::<Vec<ShikiMetaData>, anyhow::Error>(
        related
            .animes
            .into_iter()
            .map(|anime| {
                ShikiMetaData::new(
                    anime.id,
                    anime.name,
                    anime.episodes,
                    anime.episodes_aired,
                    anime.status,
                    anime.kind,
                    anime.user_rate,
                    host.to_owned(),
                    None,
                )
            })
            .collect(),
    )
}

pub fn expand(state: &mut PluginState, mpv: &mut Handle, url: &str, host: &str) -> Result<()> {
    let has_kawai_session = state
        .jar()
        .cookies(&Url::from_str(&format!("https://{host}"))?)
        .map(|cookies| cookies.to_str().map(|s| s.contains("_kawai_session")))
        .transpose()?
        .unwrap_or(false);

    let animes = if has_kawai_session {
        let (animes, user_id) = state.runtime().block_on(async {
            tokio::join!(
                fetch_animes(state.client(), state.config(), url, host),
                fetch_user_id(state.client(), host)
            )
        });

        let (mut animes, user_id) = (animes?, user_id?);

        if let Some(user_id) = user_id {
            for anime in &mut animes {
                anime.user_id = Some(user_id);
            }
        }

        animes
    } else {
        state
            .runtime()
            .block_on(fetch_animes(state.client(), state.config(), url, host))?
    };

    let current_index: i64 = mpv.get_playlist_pos()?;
    let mut insert_index = current_index + 1;
    let mut seek_index: Option<i64> = None;

    for anime in animes {
        let key = format!("{host}/{}", anime.id);

        if let Entry::Vacant(vacant_entry) = state.metadata_mut().entry(key.clone()) {
            vacant_entry.insert(MetaData::Shiki(anime.clone()));
        }

        let episodes = if anime.status == AnimeStatus::Ongoing {
            anime.episodes_aired
        } else {
            anime.episodes
        };

        for episode in 1..=episodes {
            let mut media_title = if episodes > 1 {
                format!("{} - Episode {}", anime.name, episode)
            } else {
                format!("{} - {}", anime.name, anime.kind.to_str())
            };

            if let Some(marker) =
                anime
                    .user_rate
                    .as_ref()
                    .and_then(|rate| match (rate.episodes >= episode, &rate.status) {
                        (true, _) => Some(COMPLETED_CHAR),
                        (false, UserRateStatus::Watching) => Some(WATCHING_CHAR),
                        (false, UserRateStatus::Rewatching) => Some(REWATCHING_CHAR),
                        (false, UserRateStatus::Dropped) => Some(DROPPED_CHAR),
                        (false, UserRateStatus::OnHold) => Some(ONHOLD_CHAR),
                        (false, UserRateStatus::Planned) => Some(PLANNED_CHAR),
                        _ => None,
                    })
            {
                media_title.push(' ');
                media_title.push(marker);
            }

            if seek_index.is_none() && !media_title.ends_with(COMPLETED_CHAR) {
                seek_index = Some(insert_index - 1);
            }

            let payload = Payload::new(key.clone(), episode);
            mpv.loadfile_insert_at(&media_title, &insert_index.to_string(), &payload.encode()?)?;

            insert_index += 1;
        }
    }

    mpv.playlist_remove(current_index)?;

    if let Some(seek_index) = seek_index {
        mpv.set_playlist_pos(&seek_index.to_string())?;
    }

    Ok(())
}
