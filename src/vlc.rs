//! Owned, dynamically loaded libVLC 3 handles. Every call runs on a blocking worker.
use anyhow::{Context, Result, bail};
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::Path,
    sync::Arc,
};
type Handle = *mut c_void;

macro_rules! api {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct Api { _library: libloading::Library, headless_audio: bool, $(pub $name: $ty,)* }
        impl Api {
            pub fn load(directory: &Path, headless_audio: bool) -> Result<Arc<Self>> {
                // The library remains owned for longer than every copied function pointer.
                let library: libloading::Library = unsafe {
                    libloading::os::windows::Library::load_with_flags(directory.join("libvlc.dll"), 0x100 | 0x1000)?
                }.into();
                unsafe { Ok(Arc::new(Self { $($name: *library.get(concat!(stringify!($name), "\0").as_bytes())?,)* _library: library, headless_audio })) }
            }
        }
    }
}
api! {
    libvlc_new: unsafe extern "C" fn(c_int, *const *const c_char) -> Handle,
    libvlc_release: unsafe extern "C" fn(Handle),
    libvlc_media_player_new: unsafe extern "C" fn(Handle) -> Handle,
    libvlc_media_player_release: unsafe extern "C" fn(Handle),
    libvlc_media_new_path: unsafe extern "C" fn(Handle, *const c_char) -> Handle,
    libvlc_media_release: unsafe extern "C" fn(Handle),
    libvlc_media_parse: unsafe extern "C" fn(Handle),
    libvlc_media_get_meta: unsafe extern "C" fn(Handle, c_int) -> *mut c_char,
    libvlc_media_get_duration: unsafe extern "C" fn(Handle) -> i64,
    libvlc_free: unsafe extern "C" fn(*mut c_void),
    libvlc_media_player_set_media: unsafe extern "C" fn(Handle, Handle),
    libvlc_media_player_play: unsafe extern "C" fn(Handle) -> c_int,
    libvlc_media_player_stop: unsafe extern "C" fn(Handle),
    libvlc_media_player_set_pause: unsafe extern "C" fn(Handle, c_int),
    libvlc_media_player_get_state: unsafe extern "C" fn(Handle) -> c_int,
    libvlc_media_player_get_time: unsafe extern "C" fn(Handle) -> i64,
    libvlc_media_player_get_length: unsafe extern "C" fn(Handle) -> i64,
    libvlc_media_player_is_seekable: unsafe extern "C" fn(Handle) -> c_int,
    libvlc_media_player_set_time: unsafe extern "C" fn(Handle, i64),
    libvlc_audio_set_volume: unsafe extern "C" fn(Handle, c_int) -> c_int,
}

/// Handles are private integers solely to move ownership between blocking jobs.
/// No handle is exposed, cloned or accessed concurrently; Api owns all DLL symbols.
pub struct Backend {
    api: Arc<Api>,
    instance: usize,
    player: usize,
}
impl Backend {
    pub fn new(api: Arc<Api>) -> Result<Self> {
        let mut arguments = vec![
            CString::new("--no-video")?,
            CString::new("--no-video-title-show")?,
            CString::new("--quiet")?,
        ];
        if api.headless_audio {
            arguments.push(CString::new("--aout=dummy")?);
        }
        let args: Vec<_> = arguments.iter().map(|s| s.as_ptr()).collect();
        unsafe {
            let instance = (api.libvlc_new)(args.len() as c_int, args.as_ptr());
            if instance.is_null() {
                bail!("VLC initialization failed / VLC 初始化失败");
            }
            let player = (api.libvlc_media_player_new)(instance);
            if player.is_null() {
                (api.libvlc_release)(instance);
                bail!("VLC player allocation failed");
            }
            Ok(Self {
                api,
                instance: instance as usize,
                player: player as usize,
            })
        }
    }
    fn media(&self, path: &str) -> Result<Handle> {
        if !Path::new(path).is_file() {
            bail!("File missing / 文件不存在: {path}");
        }
        let path = CString::new(path)?;
        let media =
            unsafe { (self.api.libvlc_media_new_path)(self.instance as Handle, path.as_ptr()) };
        if media.is_null() {
            bail!("Cannot open media");
        }
        Ok(media)
    }
    pub fn probe(&self, path: &str) -> Result<([String; 3], f64)> {
        let media = self.media(path)?;
        unsafe {
            (self.api.libvlc_media_parse)(media);
            let raw = [0, 1, 4].map(|kind| {
                let ptr = (self.api.libvlc_media_get_meta)(media, kind);
                if ptr.is_null() {
                    String::new()
                } else {
                    let text = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                    (self.api.libvlc_free)(ptr.cast());
                    text
                }
            });
            let duration = (self.api.libvlc_media_get_duration)(media).max(0) as f64 / 1000.0;
            (self.api.libvlc_media_release)(media);
            Ok((raw, duration))
        }
    }
    pub fn play(&mut self, path: &str, volume: u8) -> Result<()> {
        self.stop();
        let media = self
            .media(path)
            .context("Cannot start track / 无法开始播放")?;
        unsafe {
            (self.api.libvlc_media_player_set_media)(self.player as Handle, media);
            (self.api.libvlc_media_release)(media);
            if (self.api.libvlc_media_player_play)(self.player as Handle) == -1 {
                bail!("VLC cannot play this file / 无法播放文件");
            }
        }
        self.volume(volume);
        Ok(())
    }
    pub fn stop(&mut self) {
        unsafe {
            (self.api.libvlc_media_player_stop)(self.player as Handle);
        }
    }
    pub fn pause(&mut self, pause: bool) {
        unsafe {
            (self.api.libvlc_media_player_set_pause)(self.player as Handle, i32::from(pause));
        }
    }
    pub fn volume(&mut self, value: u8) {
        unsafe {
            (self.api.libvlc_audio_set_volume)(self.player as Handle, value.min(100) as c_int);
        }
    }
    pub fn seek(&mut self, seconds: f64) -> Result<()> {
        if unsafe { (self.api.libvlc_media_player_is_seekable)(self.player as Handle) } == 0 {
            bail!("Not seekable yet / 尚未就绪或无法定位");
        }
        unsafe {
            (self.api.libvlc_media_player_set_time)(
                self.player as Handle,
                (seconds.max(0.0) * 1000.0) as i64,
            );
        }
        Ok(())
    }
    pub fn snapshot(&self) -> (i32, f64, f64) {
        unsafe {
            (
                (self.api.libvlc_media_player_get_state)(self.player as Handle),
                (self.api.libvlc_media_player_get_time)(self.player as Handle).max(0) as f64
                    / 1000.0,
                (self.api.libvlc_media_player_get_length)(self.player as Handle).max(0) as f64
                    / 1000.0,
            )
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.stop();
        unsafe {
            (self.api.libvlc_media_player_release)(self.player as Handle);
            (self.api.libvlc_release)(self.instance as Handle);
        }
    }
}
