use kodik_shiki::{AnimeStatus, UserRate, UserRateStatus};
use serde::{Deserialize, Serialize};

mod expand;
mod mark_as_watched;
mod on_load;

pub use expand::expand;
pub use mark_as_watched::mark_as_watched;
pub use on_load::on_load;

pub const COMPLETED_CHAR: char = '✓';
pub const WATCHING_CHAR: char = '▶';
pub const REWATCHING_CHAR: char = '↻';

#[derive(Debug, Clone)]
pub struct ShikiMetaData {
    pub id: usize,
    pub name: String,
    pub episodes: usize,
    pub episodes_aired: usize,
    pub status: AnimeStatus,
    pub user_rate: Option<UserRate>,
    pub host: String,
    pub user_id: Option<usize>,
}

impl ShikiMetaData {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: usize,
        name: String,
        episodes: usize,
        episodes_aired: usize,
        status: AnimeStatus,
        user_rate: Option<UserRate>,
        host: String,
        user_id: Option<usize>,
    ) -> Self {
        Self {
            id,
            name,
            episodes,
            episodes_aired,
            status,
            user_rate,
            host,
            user_id,
        }
    }
}

#[derive(Debug, Serialize)]
enum UserRatesTargetType {
    Anime,
    // Manga,
    // VisualNovel,
}

#[derive(Debug, Deserialize)]
struct ShikiApiUsersWhoami {
    id: Option<usize>,
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
