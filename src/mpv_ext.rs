use anyhow::{Result, anyhow};

pub trait MpvResultExt<T> {
    fn mpv_context(self, context: impl Into<String>) -> Result<T>;
}

impl<T> MpvResultExt<T> for std::result::Result<T, mpv_client::Error> {
    fn mpv_context(self, context: impl Into<String>) -> Result<T> {
        self.map_err(|err| anyhow!("{}: {:?}", context.into(), err))
    }
}
