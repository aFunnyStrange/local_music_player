//! Serialized decoder actor: a bounded command queue, polling and immutable UI snapshots.
use crate::{
    model::{Mode, Track},
    vlc::{Api, Backend},
};
use anyhow::Result;
use rand::Rng;
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Default)]
pub struct Snapshot {
    pub current: Option<Track>,
    pub active: bool,
    pub paused: bool,
    pub position: f64,
    pub duration: f64,
    pub starts: u64,
    pub error: String,
}
pub enum Command {
    Play(Vec<Track>, usize),
    Toggle,
    Skip(i32),
    Seek(f64),
    Volume(u8),
    Mode(Mode),
    Shutdown,
}
pub struct Player {
    pub commands: mpsc::Sender<Command>,
    pub state: watch::Receiver<Snapshot>,
    pub task: tokio::task::JoinHandle<()>,
}

/// Pure queue transitions are independently testable without an audio device.
pub fn next_index(
    index: usize,
    length: usize,
    step: i32,
    automatic: bool,
    mode: Mode,
) -> Option<usize> {
    if length == 0 {
        return None;
    }
    if automatic && mode == Mode::RepeatOne {
        return Some(index);
    }
    if mode == Mode::Shuffle && length > 1 {
        return Some((index + rand::rng().random_range(1..length)) % length);
    }
    Some((index as i64 + step as i64).rem_euclid(length as i64) as usize)
}

struct Engine {
    backend: Backend,
    queue: Vec<Track>,
    index: usize,
    mode: Mode,
    volume: u8,
    state: Snapshot,
}
impl Engine {
    fn start(&mut self, index: usize) -> Result<()> {
        self.state.active = false;
        self.state.paused = false;
        self.state.position = 0.0;
        self.state.duration = 0.0;
        self.index = index;
        self.state.current = Some(self.queue[index].clone());
        self.backend.play(&self.queue[index].path, self.volume)?;
        self.state.active = true;
        self.state.starts += 1;
        self.state.error.clear();
        Ok(())
    }
    fn command(&mut self, command: Option<Command>) -> Result<()> {
        match command {
            Some(Command::Play(queue, index)) if index < queue.len() => {
                self.queue = queue;
                self.start(index)?;
            }
            Some(Command::Toggle) if !self.queue.is_empty() => {
                if !self.state.active {
                    self.start(self.index)?;
                } else {
                    self.state.paused = !self.state.paused;
                    self.backend.pause(self.state.paused);
                }
            }
            Some(Command::Skip(step)) => {
                if let Some(index) =
                    next_index(self.index, self.queue.len(), step, false, self.mode)
                {
                    self.start(index)?;
                }
            }
            Some(Command::Seek(value)) if self.state.active => {
                let target = value.clamp(0.0, self.state.duration.max(0.0));
                self.backend.seek(target)?;
                self.state.position = target;
            }
            Some(Command::Volume(value)) => {
                self.volume = value.min(100);
                self.backend.volume(self.volume);
            }
            Some(Command::Mode(mode)) => {
                self.mode = mode;
            }
            None if self.state.active => {
                let (state, position, duration) = self.backend.snapshot();
                self.state.position = position;
                self.state.duration = duration;
                if state == 7 {
                    anyhow::bail!("Cannot decode file / 无法解码音频文件");
                }
                if state == 6
                    && !self.state.paused
                    && let Some(index) =
                        next_index(self.index, self.queue.len(), 1, true, self.mode)
                {
                    self.start(index)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub async fn spawn(api: Arc<Api>, mode: Mode, volume: u8) -> Result<Player> {
    let backend = tokio::task::spawn_blocking(move || Backend::new(api)).await??;
    let mut engine = Engine {
        backend,
        queue: vec![],
        index: 0,
        mode,
        volume,
        state: Snapshot::default(),
    };
    let (commands, mut rx) = mpsc::channel(32);
    let (tx, state) = watch::channel(Snapshot::default());
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            let command = tokio::select! {
                command = rx.recv() => {
                    match command { None | Some(Command::Shutdown) => break, other => other }
                }
                _ = interval.tick() => None,
            };
            let is_seek = matches!(command, Some(Command::Seek(_)));
            engine = match tokio::task::spawn_blocking(move || {
                if let Err(error) = engine.command(command) {
                    tracing::error!("Playback: {error:#}");
                    engine.state.error = format!("{error:#}");
                    if !is_seek {
                        engine.backend.stop();
                        engine.state.active = false;
                        engine.state.paused = false;
                    }
                }
                engine
            })
            .await
            {
                Ok(engine) => engine,
                Err(error) => {
                    let _ = tx.send(Snapshot {
                        error: error.to_string(),
                        ..Default::default()
                    });
                    return;
                }
            };
            let _ = tx.send(engine.state.clone());
        }
        let _ = tokio::task::spawn_blocking(move || drop(engine)).await;
    });
    Ok(Player {
        commands,
        state,
        task,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_repeat_and_shuffle() {
        assert_eq!(next_index(1, 2, 1, true, Mode::RepeatAll), Some(0));
        assert_eq!(next_index(0, 2, -1, false, Mode::RepeatAll), Some(1));
        assert_eq!(next_index(1, 2, 1, true, Mode::RepeatOne), Some(1));
        assert_eq!(next_index(1, 2, 1, false, Mode::RepeatOne), Some(0));
        assert_eq!(next_index(0, 0, 1, true, Mode::RepeatAll), None);
        for _ in 0..50 {
            assert_ne!(next_index(1, 3, 1, true, Mode::Shuffle), Some(1));
        }
    }
}
