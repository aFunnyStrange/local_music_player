//! Shared application data, exclusive instance lock, explicit import and atomic saves.
use crate::model::State;
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub fn data_directory() -> PathBuf {
    std::env::var_os("SHIYIN_DATA_DIR")
        .or_else(|| std::env::var_os("SHIYIN_RUST_DATA_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
                .join("Shiyin")
        })
}
pub fn lock(directory: &Path) -> Result<File> {
    std::fs::create_dir_all(directory)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join("instance.lock"))?;
    file.try_lock_exclusive()
        .context("Another player uses this data directory / 已有播放器使用此数据目录")?;
    Ok(file)
}
pub async fn load(path: &Path) -> Result<State> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
        Err(error) => return Err(error.into()),
    };
    decode(&bytes)
}
pub fn decode(bytes: &[u8]) -> Result<State> {
    let mut state: State =
        serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))?;
    state.validate()?;
    Ok(state)
}
pub async fn save(path: PathBuf, state: State) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let mut validated = state;
        validated.validate()?;
        let parent = path.parent().context("Missing state parent")?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, &validated)?;
        temporary.write_all(b"\n")?;
        temporary.as_file().sync_all()?;
        temporary.persist(&path).map_err(|e| e.error)?;
        Ok::<_, anyhow::Error>(())
    })
    .await?
}
pub async fn import(source: &Path, destination: &Path) -> Result<State> {
    let bytes = tokio::fs::read(source).await?;
    let state = decode(&bytes)?;
    if source == destination {
        bail!("Choose a different source / 请选择另一个配置文件");
    }
    if tokio::fs::try_exists(destination).await? {
        let backup =
            destination.with_extension(format!("{}.backup.json", uuid::Uuid::new_v4().simple()));
        tokio::fs::copy(destination, backup).await?;
    }
    save(destination.to_owned(), state.clone()).await?;
    Ok(state)
}
