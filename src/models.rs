use serde::Deserialize;

pub type Playlist = Vec<PlaylistEntry>;

#[derive(Debug, Deserialize)]
pub struct PlaylistEntry {
    filename: String,
    id: i64,
}

impl PlaylistEntry {
    pub const fn new(filename: String, id: i64) -> Self {
        Self { filename, id }
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub const fn id(&self) -> i64 {
        self.id
    }
}
