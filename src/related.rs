use crate::{MpvResultExt as _, PluginState};
use anyhow::Result;
use reqwest::Url;

pub fn expand_by_related(state: &mut PluginState) -> Result<()> {
    let raw_url: String = state
        .mpv_mut()
        .get_property("stream-open-filename")
        .mpv_context("failed to get stream-open-filename")?;

    let Ok(url) = Url::parse(&raw_url) else {
        return Ok(());
    };

    let Some(host) = url.host_str() else {
        return Ok(());
    };

    let Some(host_name) = host.rsplit_once('.').map(|(lp, _rp)| lp) else {
        return Ok(());
    };

    if host_name == "shikimori" {
        let shikimori_id = kodik_shiki::extract_id(url.as_str())?;

        let direct_link = state.runtime().block_on(async {
            let _shiki_api_animes = kodik_shiki::fetch_shiki_api_animes(state.client(), url.as_str()).await?;
            let kodik_api_resp = kodik_shiki::fetch_kodik_videos(state.client(), shikimori_id).await?;
            let search_result = kodik_api_resp.find_search_result(None, None)?;
            let kodik_link = format!("http:{}", &search_result.link);
            let kodik_resp = kodik_parser::parse(state.client(), &kodik_link).await?;

            Ok::<String, anyhow::Error>(kodik_resp.links.quality_720.first().unwrap().src.clone())
        })?;

        state
            .mpv_mut()
            .set_property("stream-open-filename", direct_link)
            .mpv_context("failed to set stream-open-filename")?;
    }

    Ok(())
}
