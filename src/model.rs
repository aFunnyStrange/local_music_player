//! Versioned local library data. Playlists reference shared file identities.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Track {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub raw: [String; 3],
    pub notes: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    #[serde(rename = "列表循环", alias = "顺序播放")]
    RepeatAll,
    #[serde(rename = "单曲循环")]
    RepeatOne,
    #[serde(rename = "随机播放")]
    Shuffle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "zh_CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
}
impl Language {
    pub fn text<'a>(self, zh: &'a str, en: &'a str) -> &'a str {
        if self == Self::Chinese { zh } else { en }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloseAction {
    #[default]
    Ask,
    Tray,
    Exit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Playlist {
    pub name: String,
    pub tracks: BTreeSet<String>,
}

fn volume() -> u8 {
    60
}
fn all() -> String {
    "all".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct State {
    pub version: u32,
    pub roots: Vec<String>,
    pub favorites: BTreeSet<String>,
    #[serde(default)]
    pub names: BTreeMap<String, String>,
    #[serde(default = "volume")]
    pub volume: u8,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub close_action: CloseAction,
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub playlists: BTreeMap<String, Playlist>,
    #[serde(default = "all")]
    pub selected_playlist: String,
}
impl Default for State {
    fn default() -> Self {
        Self {
            version: 2,
            roots: vec![],
            favorites: BTreeSet::new(),
            names: BTreeMap::new(),
            volume: 60,
            mode: Mode::default(),
            close_action: CloseAction::default(),
            language: Language::default(),
            playlists: BTreeMap::new(),
            selected_playlist: all(),
        }
    }
}
impl State {
    pub fn validate(&mut self) -> Result<()> {
        if ![1, 2].contains(&self.version) || self.volume > 100 {
            bail!("Invalid settings version or volume / 配置版本或音量无效");
        }
        let mut names = BTreeSet::new();
        for (id, playlist) in &self.playlists {
            if id.is_empty()
                || id == "all"
                || playlist.name.trim().is_empty()
                || playlist.name.chars().count() > 100
                || !names.insert(crate::search::normalize(playlist.name.trim()))
            {
                bail!("Invalid or duplicate playlist / 歌单无效或重名");
            }
        }
        self.version = 2;
        if self.selected_playlist != "all" && !self.playlists.contains_key(&self.selected_playlist)
        {
            self.selected_playlist = all();
        }
        Ok(())
    }
    pub fn name_playlist(&mut self, id: Option<&str>, name: &str) -> Result<String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            bail!("Name must be 1–100 characters / 名称须为 1–100 字符");
        }
        if self.playlists.iter().any(|(key, p)| {
            Some(key.as_str()) != id
                && crate::search::normalize(&p.name) == crate::search::normalize(name)
        }) {
            bail!("Playlist name already exists / 歌单已存在");
        }
        let key = id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().simple().to_string());
        if let Some(id) = id {
            self.playlists
                .get_mut(id)
                .ok_or_else(|| anyhow::anyhow!("Unknown playlist"))?
                .name = name.into();
        } else {
            self.playlists.insert(
                key.clone(),
                Playlist {
                    name: name.into(),
                    tracks: BTreeSet::new(),
                },
            );
        }
        Ok(key)
    }
    pub fn memberships(&mut self, path: &str, selected: &BTreeSet<String>) -> Result<()> {
        if selected.iter().any(|id| !self.playlists.contains_key(id)) {
            bail!("Unknown playlist");
        }
        for (id, p) in &mut self.playlists {
            if selected.contains(id) {
                p.tracks.insert(path.into());
            } else {
                p.tracks.remove(path);
            }
        }
        Ok(())
    }
    pub fn title<'a>(&'a self, track: &'a Track) -> &'a str {
        self.names
            .get(&track.path)
            .map(String::as_str)
            .unwrap_or(&track.title)
    }
    pub fn visible(
        &self,
        tracks: &[Track],
        query: &crate::search::Query,
        favorites: bool,
    ) -> Vec<usize> {
        let membership = self.playlists.get(&self.selected_playlist);
        let mut result: Vec<_> = tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                (!favorites || self.favorites.contains(&t.path))
                    && membership.is_none_or(|p| p.tracks.contains(&t.path))
                    && query.matches(&format!(
                        "{} {} {} {}",
                        self.title(t),
                        t.artist,
                        t.album,
                        std::path::Path::new(&t.path)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                    ))
            })
            .map(|(i, _)| i)
            .collect();
        result.sort_by_cached_key(|i| crate::search::normalize(self.title(&tracks[*i])));
        result
    }
}
