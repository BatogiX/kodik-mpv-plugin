use kodik_shiki::{AnimeKind, AnimeStatus, UserRate};

mod expand;
mod mark_as_watched;
mod on_load;

pub use expand::expand;
pub use mark_as_watched::mark_as_watched;
pub use on_load::on_load;

pub const COMPLETED_CHAR: char = '✓';
pub const DROPPED_CHAR: char = '✕';
pub const ONHOLD_CHAR: char = '‖';
pub const PLANNED_CHAR: char = '＋';
pub const WATCHING_CHAR: char = '▶';
pub const REWATCHING_CHAR: char = '↺';

#[derive(Debug, Clone)]
pub struct ShikiMetaData {
    pub id: usize,
    pub name: String,
    pub episodes: usize,
    pub episodes_aired: usize,
    pub status: AnimeStatus,
    pub kind: AnimeKind,
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
        kind: AnimeKind,
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
            kind,
            user_rate,
            host,
            user_id,
        }
    }
}
