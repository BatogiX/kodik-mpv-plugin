use std::collections::hash_map::Entry;
use std::str::FromStr as _;

use crate::config::Config;
use crate::mpv_ext::MpvExt;
use crate::shiki::{COMPLETED_CHAR, REWATCHING_CHAR, WATCHING_CHAR};
use crate::{
    hooks::{MetaData, Payload},
    shiki::{ShikiApiUsersWhoami, ShikiMetaData},
};
use anyhow::Result;
use kodik_shiki::{AnimeStatus, Related, UserRateStatus};
use kodik_utils::GET;
use mpv_client::Handle;
use reqwest::cookie::CookieStore as _;
use reqwest::{Client, Url};

use crate::{config::RelatedMode, state::PluginState};

async fn fetch_user_id(client: &Client, host: &str) -> Result<Option<usize>> {
    let user_id = client
        .fetch_as_json::<ShikiApiUsersWhoami>(&format!("https://{host}/api/users/whoami"))
        .await?
        .id;

    anyhow::Ok(user_id)
}

async fn fetch_animes(client: &Client, config: &Config, url: &str, host: &str) -> Result<Vec<ShikiMetaData>> {
    let shiki_api_animes = kodik_shiki::fetch_shiki_api_animes(client, url).await?;

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
            let mut media_title = if anime.episodes == 1 {
                format!("{} - Movie", anime.name)
            } else {
                format!("{} - Episode {}", anime.name, episode)
            };

            if let Some(user_rate) = anime.user_rate {
                if user_rate.episodes >= episode {
                    media_title.push(' ');
                    media_title.push(COMPLETED_CHAR);
                } else {
                    match user_rate.status {
                        UserRateStatus::Watching => {
                            media_title.push(' ');
                            media_title.push(WATCHING_CHAR);
                        }
                        UserRateStatus::Rewatching => {
                            media_title.push(' ');
                            media_title.push(REWATCHING_CHAR);
                        }
                        _ => {}
                    }
                }
            }

            if seek_index.is_none() && !media_title.ends_with(COMPLETED_CHAR) {
                seek_index = Some(insert_index - 1);
            }

            let payload = Payload::new(key.clone(), episode);
            mpv.loadfile_insert_at(&media_title, &insert_index.to_string(), &payload.encode(&media_title)?)?;

            insert_index += 1;
        }
    }

    mpv.playlist_remove(current_index)?;

    if let Some(seek_index) = seek_index {
        mpv.set_playlist_pos(&seek_index.to_string())?;
    }

    Ok(())
}
