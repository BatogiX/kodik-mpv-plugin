use crate::{PluginState, models::PlaylistEntry, shiki};
use anyhow::Result;
use mpv_client::Node;
use reqwest::Url;

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
        let Ok(url) = Url::parse(filename) else {
            continue;
        };

        let Some(host) = url.host_str() else {
            continue;
        };

        let Some(host_name) = host.rsplit_once('.').map(|(lp, _rp)| lp) else {
            continue;
        };

        match host_name {
            "shikimori" => shiki::expand_by_related(state, url.as_str(), host, index)?,
            "myanimelist" => {}
            _ => {}
        }
    }

    Ok(())
}
