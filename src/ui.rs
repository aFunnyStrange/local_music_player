//! Native immediate-mode UI; all decoder/filesystem work is sent to async owners.
use eframe::egui::{self, ViewportCommand};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use shiyin_rs::{
    library,
    model::{CloseAction, Language, Mode, State, Track},
    playback::{Command, Snapshot},
    search::Query,
    services::{Event, Write},
    vlc::Api,
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};
use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

pub struct Desktop {
    scan_progress: Arc<library::Progress>,
    lifecycle: Option<bool>,
    window: isize,
    pub state: State,
    pub final_state: Arc<Mutex<State>>,
    pub runtime: tokio::runtime::Handle,
    pub api: Arc<Api>,
    pub playback: mpsc::Sender<Command>,
    pub snapshot: watch::Receiver<Snapshot>,
    pub writes: mpsc::Sender<Write>,
    pub events: mpsc::Receiver<Event>,
    pub sender: mpsc::Sender<Event>,
    pub cancel: Arc<AtomicBool>,
    pub jobs: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    tracks: Vec<Track>,
    visible: Vec<usize>,
    query: String,
    favorites: bool,
    selected: Option<String>,
    status: String,
    scanning: bool,
    picking_folder: bool,
    importing: bool,
    dirty: Option<Instant>,
    folders: bool,
    playlists: bool,
    membership: bool,
    info: bool,
    rename: bool,
    rename_text: String,
    playlist_name: String,
    playlist_edit: Option<String>,
    memberships: BTreeSet<String>,
    close_dialog: bool,
    close_settings: bool,
    remember: bool,
    hidden: bool,
    exiting: bool,
    tray: Option<TrayIcon>,
    menu_items: Vec<MenuItem>,
    seek_value: f64,
    seek_track: Option<String>,
    seek_drag: bool,
    probe: Option<Arc<AtomicBool>>,
    probe_step: u8,
}

pub struct Inputs {
    pub scan_progress: Arc<library::Progress>,
    pub lifecycle: Option<bool>,
    pub probe: Option<Arc<AtomicBool>>,
    pub state: State,
    pub final_state: Arc<Mutex<State>>,
    pub runtime: tokio::runtime::Handle,
    pub api: Arc<Api>,
    pub playback: mpsc::Sender<Command>,
    pub snapshot: watch::Receiver<Snapshot>,
    pub writes: mpsc::Sender<Write>,
    pub events: mpsc::Receiver<Event>,
    pub sender: mpsc::Sender<Event>,
    pub cancel: Arc<AtomicBool>,
    pub jobs: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}
impl Desktop {
    pub fn new(cc: &eframe::CreationContext<'_>, inputs: Inputs, font: Option<Vec<u8>>) -> Self {
        if let Some(font) = font {
            let mut definitions = egui::FontDefinitions::default();
            // msyh.ttc contains YaHei at index 0 and the Tk-matching YaHei UI at index 1.
            let ui_face = font.starts_with(b"ttcf")
                && font
                    .get(8..12)
                    .is_some_and(|count| u32::from_be_bytes(count.try_into().unwrap()) > 1);
            let mut font_data = egui::FontData::from_owned(font);
            font_data.index = u32::from(ui_face);
            definitions
                .font_data
                .insert("cjk".into(), Arc::new(font_data));
            definitions
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk".into());
            definitions
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk".into());
            cc.egui_ctx.set_fonts(definitions);
        }
        cc.egui_ctx.set_theme(egui::Theme::Light);
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut style = (*cc.egui_ctx.style()).clone();
        for kind in [
            egui::TextStyle::Body,
            egui::TextStyle::Button,
            egui::TextStyle::Monospace,
        ] {
            style
                .text_styles
                .insert(kind, egui::FontId::proportional(12.0));
        }
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(26.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.interact_size.y = 32.0;
        style.spacing.combo_width = 160.0;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::WHITE;
        style.visuals.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.45 };
        style.visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_gray(190));
        cc.egui_ctx.set_style_of(egui::Theme::Light, style);
        let mut this = Self {
            scan_progress: inputs.scan_progress,
            lifecycle: inputs.lifecycle,
            window: match cc.window_handle().expect("native window").as_raw() {
                RawWindowHandle::Win32(handle) => handle.hwnd.get(),
                _ => unreachable!("Windows only"),
            },
            state: inputs.state,
            final_state: inputs.final_state,
            runtime: inputs.runtime,
            api: inputs.api,
            playback: inputs.playback,
            snapshot: inputs.snapshot,
            writes: inputs.writes,
            events: inputs.events,
            sender: inputs.sender,
            cancel: inputs.cancel,
            jobs: inputs.jobs,
            tracks: vec![],
            visible: vec![],
            query: String::new(),
            favorites: false,
            selected: None,
            status: String::new(),
            scanning: false,
            picking_folder: false,
            importing: false,
            dirty: None,
            folders: false,
            playlists: false,
            membership: false,
            info: false,
            rename: false,
            rename_text: String::new(),
            playlist_name: String::new(),
            playlist_edit: None,
            memberships: BTreeSet::new(),
            close_dialog: false,
            close_settings: false,
            remember: false,
            hidden: false,
            exiting: false,
            tray: None,
            menu_items: vec![],
            seek_value: 0.0,
            seek_track: None,
            seek_drag: false,
            probe: inputs.probe,
            probe_step: 0,
        };
        if this.probe.is_some() {
            let context = cc.egui_ctx.clone();
            let sender = this.sender.clone();
            let window = this.window;
            let deadline = if this.lifecycle.is_some() { 120 } else { 20 };
            this.runtime.spawn(async move {
                tokio::time::sleep(Duration::from_secs(deadline)).await;
                wake_window(window);
                context.send_viewport_cmd(ViewportCommand::Visible(true));
                let _ = sender.send(Event::Exit).await;
                context.request_repaint();
            });
        }
        this.refresh();
        this
    }
    fn dirty(&mut self) {
        *self.final_state.lock().unwrap() = self.state.clone();
        self.dirty = Some(Instant::now());
    }
    fn filter(&mut self) {
        self.visible = self
            .state
            .visible(&self.tracks, &Query::new(&self.query), self.favorites);
    }
    fn command(&mut self, command: Command) {
        if self.playback.try_send(command).is_err() {
            self.status = "Player busy; retry / 播放器忙，请重试".into();
        }
    }
    fn selected_track(&self) -> Option<&Track> {
        self.selected
            .as_ref()
            .and_then(|path| self.tracks.iter().find(|t| &t.path == path))
    }
    fn play_selected(&mut self) {
        let queue: Vec<_> = self
            .visible
            .iter()
            .map(|i| self.tracks[*i].clone())
            .collect();
        if let Some(index) = queue
            .iter()
            .position(|t| Some(&t.path) == self.selected.as_ref())
        {
            self.command(Command::Play(queue, index));
        }
    }
    fn toggle(&mut self) {
        if self.snapshot.borrow().current.is_none() {
            self.play_selected();
        } else {
            self.command(Command::Toggle);
        }
    }
    fn refresh(&mut self) {
        if self.scanning {
            return;
        }
        self.cancel.store(false, Ordering::Relaxed);
        self.scanning = true;
        self.scan_progress.parsed.store(0, Ordering::Relaxed);
        self.scan_progress.cancelled.store(false, Ordering::Relaxed);
        self.scan_progress.finished.store(false, Ordering::Release);
        let progress = self.scan_progress.clone();
        let (roots, api, cancel, sender) = (
            self.state.roots.clone(),
            self.api.clone(),
            self.cancel.clone(),
            self.sender.clone(),
        );
        let job = self.runtime.spawn(async move {
            let event = match library::scan_with_progress(roots, api, cancel, progress).await {
                Ok(result) => Event::Scanned(result),
                Err(e) => Event::ScanError(format!("{e:#}")),
            };
            let _ = sender.send(event).await;
        });
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|job| !job.is_finished());
        jobs.push(job);
    }
    fn choose_folder(&mut self) {
        if self.picking_folder {
            return;
        }
        self.picking_folder = true;
        let sender = self.sender.clone();
        self.runtime.spawn(async move {
            if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                let _ = sender.send(Event::Folder(folder.path().to_owned())).await;
            } else {
                let _ = sender.send(Event::FolderCancelled).await;
            }
        });
    }
    fn import(&mut self) {
        if self.scanning {
            self.status = "Cancel or finish scanning before importing / 请先完成或取消扫描".into();
            return;
        }
        self.importing = true;
        let (writes, sender) = (self.writes.clone(), self.sender.clone());
        self.runtime.spawn(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("Library settings (*.json)", &["json"])
                .pick_file()
                .await
            {
                if writes
                    .send(Write::Import(file.path().to_owned()))
                    .await
                    .is_err()
                {
                    let _ = sender
                        .send(Event::Error("Import service closed".into()))
                        .await;
                }
            } else {
                let _ = sender
                    .send(Event::Error("Import cancelled / 已取消导入".into()))
                    .await;
            }
        });
    }
    fn process(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Restore => self.restore(ctx),
                Event::Exit => self.exit(ctx),
                Event::Scanned(scan) => {
                    self.scanning = false;
                    if scan.cancelled {
                        self.status =
                            "Scan cancelled; old library retained / 扫描已取消，保留原列表".into();
                    } else {
                        self.tracks = scan.tracks;
                        self.filter();
                        self.status = if scan.errors.is_empty() {
                            String::new()
                        } else {
                            format!(
                                "{} scan errors / 扫描错误\n{}",
                                scan.errors.len(),
                                scan.errors.join("\n")
                            )
                        };
                        for error in scan.errors {
                            tracing::warn!("Scan: {error}");
                        }
                    }
                }
                Event::Folder(folder) => {
                    self.picking_folder = false;
                    let key = library::identity(&folder);
                    if !self.state.roots.contains(&key) {
                        self.state.roots.push(key);
                        self.dirty();
                    }
                    if !self.scanning {
                        self.refresh();
                    } else {
                        self.status = "Folder added; refresh after current scan / 已添加目录，当前扫描后请刷新".into();
                    }
                }
                Event::Imported(state) => {
                    self.importing = false;
                    self.state = state;
                    self.dirty();
                    self.command(Command::Mode(self.state.mode));
                    self.command(Command::Volume(self.state.volume));
                    self.filter();
                    self.refresh();
                    self.status = "Imported; original settings and backup retained / 已导入，原配置与备份已保留".into();
                }
                Event::Saved => {}
                Event::FolderCancelled => {
                    self.picking_folder = false;
                }
                Event::ScanError(error) => {
                    self.scanning = false;
                    self.status = error;
                }
                Event::Error(error) => {
                    self.importing = false;
                    self.status = error;
                }
            }
        }
        if self
            .dirty
            .is_some_and(|time| time.elapsed() >= Duration::from_millis(500))
            && !self.importing
            && self
                .writes
                .try_send(Write::Save(self.state.clone()))
                .is_ok()
        {
            self.dirty = None;
        }
    }
    fn tray(&mut self) -> anyhow::Result<()> {
        if self.tray.is_none() {
            let image = image::load_from_memory(include_bytes!("../assets/logo.ico"))?.into_rgba8();
            let icon =
                tray_icon::Icon::from_rgba(image.as_raw().clone(), image.width(), image.height())?;
            let menu = Menu::new();
            self.menu_items = ["Open", "Pause", "Previous", "Next", "Exit"]
                .map(|s| MenuItem::new(s, true, None))
                .into();
            for item in &self.menu_items {
                menu.append(item)?;
            }
            self.tray = Some(
                TrayIconBuilder::new()
                    .with_icon(icon)
                    .with_tooltip("拾音 · Shiyin")
                    .with_menu(Box::new(menu))
                    .with_menu_on_left_click(false)
                    .build()?,
            );
        }
        self.tray.as_ref().unwrap().set_visible(true)?;
        Ok(())
    }
    fn hide(&mut self, ctx: &egui::Context) {
        match self.tray() {
            Ok(()) => {
                self.install_tray_handlers(ctx);
                self.hidden = true;
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
            Err(error) => {
                self.status = format!("Tray unavailable / 托盘不可用: {error}");
            }
        }
    }
    fn install_tray_handlers(&self, ctx: &egui::Context) {
        let window = self.window;
        let context = ctx.clone();
        let events = self.sender.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                wake_window(window);
                context.send_viewport_cmd(ViewportCommand::Visible(true));
                let _ = events.try_send(Event::Restore);
                context.request_repaint();
            }
        }));
        let ids: Vec<_> = self
            .menu_items
            .iter()
            .map(|item| item.id().clone())
            .collect();
        let commands = self.playback.clone();
        let events = self.sender.clone();
        let context = ctx.clone();
        let state = self.snapshot.clone();
        let queue: Vec<_> = self
            .visible
            .iter()
            .map(|i| self.tracks[*i].clone())
            .collect();
        let selected = queue
            .iter()
            .position(|t| Some(&t.path) == self.selected.as_ref());
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let command = match ids.iter().position(|id| id == &event.id) {
                Some(0) => {
                    wake_window(window);
                    context.send_viewport_cmd(ViewportCommand::Visible(true));
                    let _ = events.try_send(Event::Restore);
                    None
                }
                Some(1) => {
                    if state.borrow().current.is_none() {
                        selected.map(|index| Command::Play(queue.clone(), index))
                    } else {
                        Some(Command::Toggle)
                    }
                }
                Some(2) => Some(Command::Skip(-1)),
                Some(3) => Some(Command::Skip(1)),
                Some(4) => {
                    wake_window(window);
                    context.send_viewport_cmd(ViewportCommand::Visible(true));
                    let _ = events.try_send(Event::Exit);
                    None
                }
                _ => None,
            };
            if let Some(command) = command {
                let _ = commands.try_send(command);
            }
            context.request_repaint();
        }));
    }
    fn restore(&mut self, ctx: &egui::Context) {
        self.hidden = false;
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        if let Some(tray) = &self.tray {
            let _ = tray.set_visible(false);
        }
    }
    fn exit(&mut self, ctx: &egui::Context) {
        self.exiting = true;
        self.cancel.store(true, Ordering::Relaxed);
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
    fn update_tray_labels(&mut self, snap: &Snapshot) {
        let l = self.state.language;
        let toggle = if snap.paused {
            l.text("继续", "Resume")
        } else if snap.active {
            l.text("暂停", "Pause")
        } else {
            l.text("播放", "Play")
        };
        for (item, text) in self.menu_items.iter().zip([
            l.text("打开播放器", "Open player"),
            toggle,
            l.text("上一首", "Previous"),
            l.text("下一首", "Next"),
            l.text("退出", "Exit"),
        ]) {
            if item.text() != text {
                item.set_text(text);
            }
        }
    }
}

impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.process(ctx);
        let snap = self.snapshot.borrow().clone();
        self.update_tray_labels(&snap);
        ctx.request_repaint_after(Duration::from_millis(if self.scanning {
            100
        } else if snap.active && !snap.paused {
            200
        } else {
            500
        }));
        if ctx.input(|i| i.viewport().close_requested()) && !self.exiting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            if !self.importing {
                match self.state.close_action {
                    CloseAction::Ask => self.close_dialog = true,
                    CloseAction::Tray => self.hide(ctx),
                    CloseAction::Exit => self.exit(ctx),
                }
            }
        }
        let l = self.state.language;
        let panel_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(244, 244, 244))
            .inner_margin(egui::Margin::symmetric(16, 12));
        egui::TopBottomPanel::top("toolbar").frame(panel_frame).show(ctx, |ui| {
            ui.add_enabled_ui(!self.importing, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(90.0, 38.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.label(egui::RichText::new(l.text("拾音", "Shiyin")).size(26.0).strong()); });
                    if ui.button(l.text("添加文件夹", "Add folder")).clicked() { self.choose_folder(); }
                    if ui.button(l.text("管理目录", "Folders")).clicked() { self.folders = true; }
                    if ui.add_enabled(!self.scanning, egui::Button::new(l.text("刷新", "Refresh"))).clicked() { self.refresh(); }
                    if ui.add_enabled(self.scanning, egui::Button::new(l.text("取消扫描", "Cancel scan"))).clicked() { self.cancel.store(true, Ordering::Relaxed); }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(l.text("English", "简体中文")).clicked() {
                        self.state.language = if l == Language::Chinese { Language::English } else { Language::Chinese }; self.dirty();
                    }
                    if ui.checkbox(&mut self.favorites, l.text("只看收藏", "Favorites only")).changed() { self.filter(); }
                    });
                });
            });
            if self.importing { ui.label(l.text("等待导入…", "Import in progress…")); }
            ui.add_space(6.0);
            ui.add_enabled_ui(!self.importing && !self.membership && !self.rename && !self.info, |ui| {
                ui.horizontal(|ui| {
                    ui.label(l.text("搜索歌曲 / 歌手 / 专辑 / 文件名", "Search title / artist / album / filename"));
                    let response = ui.add_sized([ (ui.available_width() - 94.0).max(120.0), 28.0 ], egui::TextEdit::singleline(&mut self.query));
                    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) { response.request_focus(); }
                    if response.changed() { self.filter(); }
                    if ui.add_sized([80.0, 32.0], egui::Button::new(l.text("清空", "Clear"))).clicked() { self.query.clear(); self.filter(); }
                });
                ui.weak(egui::RichText::new(l.text("空格：同时匹配    |：任选其一    !live：排除    \"完整短语\"    *.flac：格式", "Space: match all    |: match any    !live: exclude    \"exact phrase\"    *.flac: format")).size(12.0));
                ui.horizontal_wrapped(|ui| {
                    ui.label(l.text("歌单", "Playlist"));
                    let current = self.state.playlists.get(&self.state.selected_playlist).map(|p| p.name.as_str()).unwrap_or(l.text("默认歌单 · 全部音乐", "Default · All music"));
                    let old = self.state.selected_playlist.clone();
                    egui::ComboBox::from_id_salt("playlist").width(230.0).selected_text(current).show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.state.selected_playlist, "all".into(), l.text("默认歌单 · 全部音乐", "Default · All music"));
                        for (id, p) in &self.state.playlists { ui.selectable_value(&mut self.state.selected_playlist, id.clone(), &p.name); }
                    });
                    if old != self.state.selected_playlist { self.filter(); self.dirty(); }
                    if ui.button(l.text("管理歌单", "Manage playlists")).clicked() { self.playlists = true; }
                            if ui
                                .button(l.text("加入 / 移出歌单", "Playlist membership"))
                                .clicked()
                                && let Some(path) = self.selected.as_ref()
                            {
                                self.memberships = self
                                    .state
                                    .playlists
                                    .iter()
                                    .filter(|(_, p)| p.tracks.contains(path))
                                    .map(|(id, _)| id.clone())
                                    .collect();
                                self.membership = true;
                            }

                });
            });
        });
        egui::TopBottomPanel::bottom("transport")
            .frame(panel_frame)
            .show(ctx, |ui| {
                ui.add_enabled_ui(
                    !self.importing && !self.membership && !self.rename && !self.info,
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .button(l.text("收藏 / 取消收藏", "Toggle favorite"))
                                .clicked()
                                && let Some(path) = self.selected.clone()
                            {
                                if !self.state.favorites.remove(&path) {
                                    self.state.favorites.insert(path);
                                }
                                self.filter();
                                self.dirty();
                            }
                            if ui.button(l.text("修改显示名称", "Display name")).clicked()
                                && let Some(track) = self.selected_track()
                            {
                                self.rename_text = self.state.title(track).into();
                                self.rename = true;
                            }
                            if ui.button(l.text("文件信息", "File info")).clicked() {
                                self.info = true;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button(l.text("关闭设置", "Close behavior")).clicked()
                                    {
                                        self.close_settings = true;
                                        self.close_dialog = true;
                                    }
                                },
                            );
                        });
                    },
                );
                ui.add_space(6.0);
                if let Some(track) = &snap.current {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            self.state.title(track),
                            track.artist
                        ))
                        .size(16.0),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(l.text("还没有播放歌曲", "No track playing"))
                            .size(16.0),
                    );
                }
                let current = snap.current.as_ref().map(|t| t.path.clone());
                if self.seek_track != current {
                    self.seek_drag = false;
                    self.seek_track = current;
                }
                if !self.seek_drag {
                    self.seek_value = snap.position;
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().interact_size.y = 20.0;
                    ui.spacing_mut().slider_width = (ui.available_width() - 140.0).max(100.0);
                    let response = ui.add_enabled(
                        snap.active && snap.duration > 0.0,
                        egui::Slider::new(&mut self.seek_value, 0.0..=snap.duration.max(1.0))
                            .show_value(false),
                    );
                    if response.drag_started() {
                        self.seek_drag = true;
                    }
                    if response.drag_stopped() || (response.changed() && !response.dragged()) {
                        self.seek_drag = false;
                        self.command(Command::Seek(self.seek_value));
                    }
                    ui.label(format!(
                        "{} / {}",
                        clock(self.seek_value),
                        clock(snap.duration)
                    ));
                });
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_sized(
                            [105.0, 37.0],
                            egui::Button::new(l.text("上一首", "Previous")),
                        )
                        .clicked()
                    {
                        self.command(Command::Skip(-1));
                    }
                    let label = if snap.paused {
                        l.text("继续", "Resume")
                    } else if snap.active {
                        l.text("暂停", "Pause")
                    } else {
                        l.text("播放", "Play")
                    };
                    if ui
                        .add_sized([105.0, 37.0], egui::Button::new(label))
                        .clicked()
                    {
                        self.toggle();
                    }
                    if ui
                        .add_sized([105.0, 37.0], egui::Button::new(l.text("下一首", "Next")))
                        .clicked()
                    {
                        self.command(Command::Skip(1));
                    }
                    let old = self.state.mode;
                    egui::ComboBox::from_id_salt("mode")
                        .selected_text(mode_label(old, l))
                        .show_ui(ui, |ui| {
                            for mode in [Mode::RepeatAll, Mode::RepeatOne, Mode::Shuffle] {
                                ui.selectable_value(
                                    &mut self.state.mode,
                                    mode,
                                    mode_label(mode, l),
                                );
                            }
                        });
                    if old != self.state.mode {
                        self.command(Command::Mode(self.state.mode));
                        self.dirty();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().interact_size.y = 20.0;
                        ui.spacing_mut().slider_width = 110.0;
                        let response = ui.add(
                            egui::Slider::new(&mut self.state.volume, 0..=100).show_value(false),
                        );
                        ui.label(l.text("音量", "Volume"));
                        if response.changed() {
                            self.command(Command::Volume(self.state.volume));
                            self.dirty();
                        }
                    });
                });
                ui.label(format!(
                    "{} / {} {}",
                    self.visible.len(),
                    self.tracks.len(),
                    l.text("首", "tracks")
                ));
                if self.scanning {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!(
                            "{} {}",
                            l.text("已解析", "Parsed"),
                            self.scan_progress.parsed.load(Ordering::Relaxed)
                        ));
                    });
                }
                if !snap.error.is_empty() {
                    ui.colored_label(egui::Color32::DARK_RED, &snap.error);
                }
                if !self.status.is_empty() {
                    egui::ScrollArea::vertical()
                        .max_height(70.0)
                        .show(ui, |ui| {
                            ui.label(&self.status);
                        });
                }
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(244, 244, 244))
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ctx, |ui| {
                ui.add_enabled_ui(
                    !self.importing && !self.membership && !self.rename && !self.info,
                    |ui| {
                        let table_width = (ui.available_width() - 16.0).max(960.0);
                        let extra = (table_width - 960.0) / 4.0;
                        let widths = [
                            52.0,
                            250.0 + extra,
                            140.0 + extra,
                            150.0 + extra,
                            68.0,
                            300.0 + extra,
                        ];
                        let table_width: f32 = widths.iter().sum();
                        egui::Frame::new()
                            .fill(egui::Color32::WHITE)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(180)))
                            .show(ui, |ui| {
                                egui::ScrollArea::horizontal()
                                    .id_salt("library_columns")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.set_min_width(table_width);
                                        let headings = [
                                            l.text("收藏", "Favorite"),
                                            l.text("歌曲", "Title"),
                                            l.text("歌手", "Artist"),
                                            l.text("专辑", "Album"),
                                            l.text("时长", "Duration"),
                                            l.text("原文件名", "Original filename"),
                                        ];
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 0.0;
                                            for (width, heading) in widths.iter().zip(headings) {
                                                ui.add_sized(
                                                    [*width, 26.0],
                                                    egui::Label::new(
                                                        egui::RichText::new(heading).strong(),
                                                    ),
                                                );
                                            }
                                        });
                                        ui.separator();
                                        let mut play = false;
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        egui::ScrollArea::vertical()
                                            .id_salt("library_rows")
                                            .auto_shrink([false, false])
                                            .show_rows(
                                                ui,
                                                29.0,
                                                self.visible.len(),
                                                |ui, range| {
                                                    ui.spacing_mut().item_spacing.y = 0.0;
                                                    for row in range {
                                                        let track = &self.tracks[self.visible[row]];
                                                        let (rect, _) = ui.allocate_exact_size(
                                                            egui::vec2(table_width, 29.0),
                                                            egui::Sense::hover(),
                                                        );
                                                        let selected = self.selected.as_ref()
                                                            == Some(&track.path);
                                                        let fill = if selected {
                                                            egui::Color32::from_rgb(217, 234, 250)
                                                        } else if row % 2 == 1 {
                                                            egui::Color32::from_gray(249)
                                                        } else {
                                                            egui::Color32::WHITE
                                                        };
                                                        ui.painter().rect_filled(rect, 0.0, fill);
                                                        let file =
                                                            std::path::Path::new(&track.path)
                                                                .file_name()
                                                                .unwrap_or_default()
                                                                .to_string_lossy();
                                                        let duration = clock(track.duration);
                                                        let values = [
                                                            if self
                                                                .state
                                                                .favorites
                                                                .contains(&track.path)
                                                            {
                                                                "♥"
                                                            } else {
                                                                ""
                                                            },
                                                            self.state.title(track),
                                                            &track.artist,
                                                            &track.album,
                                                            &duration,
                                                            &file,
                                                        ];
                                                        let mut x = rect.left();
                                                        for (width, value) in
                                                            widths.iter().zip(values)
                                                        {
                                                            let cell = egui::Rect::from_min_size(
                                                                egui::pos2(x + 8.0, rect.top()),
                                                                egui::vec2(*width - 16.0, 29.0),
                                                            );
                                                            let mut child = ui.new_child(
                                                                egui::UiBuilder::new()
                                                                    .max_rect(cell)
                                                                    .layout(
                                                                        egui::Layout::left_to_right(
                                                                            egui::Align::Center,
                                                                        ),
                                                                    ),
                                                            );
                                                            child
                                                                .add(
                                                                    egui::Label::new(value)
                                                                        .selectable(false)
                                                                        .truncate(),
                                                                )
                                                                .on_hover_text(value);
                                                            x += width;
                                                        }
                                                        let response = ui.interact(
                                                            rect,
                                                            ui.id().with(&track.path),
                                                            egui::Sense::click(),
                                                        );
                                                        if response.clicked() {
                                                            self.selected =
                                                                Some(track.path.clone());
                                                        }
                                                        if response.double_clicked() {
                                                            self.selected =
                                                                Some(track.path.clone());
                                                            play = true;
                                                        }
                                                        response.on_hover_text(&track.path);
                                                    }
                                                },
                                            );
                                        if play {
                                            self.play_selected();
                                        }
                                    });
                            });
                    },
                );
            });
        if !ctx.wants_keyboard_input()
            && ctx.input(|i| !i.modifiers.alt && !i.modifiers.ctrl && !i.modifiers.command)
        {
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.toggle();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.play_selected();
            }
        }
        self.dialogs(ctx);
        if self.probe.is_some() {
            self.probe_ui(ctx, &snap);
        }
    }
}

fn clock(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

impl Drop for Desktop {
    fn drop(&mut self) {
        TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn mode_label(mode: Mode, l: Language) -> &'static str {
    match mode {
        Mode::RepeatAll => l.text("列表循环", "Repeat all"),
        Mode::RepeatOne => l.text("单曲循环", "Repeat one"),
        Mode::Shuffle => l.text("随机播放", "Shuffle"),
    }
}

impl Desktop {
    fn dialogs(&mut self, ctx: &egui::Context) {
        let l = self.state.language;
        if self.folders {
            let mut open = true;
            egui::Window::new(l.text(
                "音乐目录（不删除文件）",
                "Music folders (files stay in place)",
            ))
            .open(&mut open)
            .show(ctx, |ui| {
                if ui
                    .add_enabled(
                        !self.importing && !self.scanning,
                        egui::Button::new(l.text("导入曲库配置", "Import library settings")),
                    )
                    .clicked()
                {
                    self.import();
                }
                ui.add_space(8.0);
                let mut remove = None;
                for (i, folder) in self.state.roots.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.button(l.text("移除", "Remove")).clicked() {
                            remove = Some(i);
                        }
                        ui.label(folder);
                    });
                }
                if let Some(i) = remove {
                    self.state.roots.remove(i);
                    self.dirty();
                    if !self.scanning {
                        self.refresh();
                    } else {
                        self.status = "Refresh after scan / 扫描完成后请刷新".into();
                    }
                }
            });
            self.folders = open;
        }
        if self.playlists {
            let mut open = true;
            egui::Window::new(l.text("管理歌单", "Manage playlists"))
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(l.text(
                        "删除歌单不会删除音频文件。",
                        "Deleting a playlist never deletes audio.",
                    ));
                    let items: Vec<_> = self
                        .state
                        .playlists
                        .iter()
                        .map(|(id, p)| (id.clone(), p.name.clone(), p.tracks.len()))
                        .collect();
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for (id, name, count) in items {
                                ui.horizontal(|ui| {
                                    if ui
                                        .selectable_label(
                                            self.playlist_edit.as_ref() == Some(&id),
                                            format!("{name} ({count})"),
                                        )
                                        .clicked()
                                    {
                                        self.playlist_edit = Some(id.clone());
                                        self.playlist_name = name;
                                    }
                                    if ui.button(l.text("删除", "Delete")).clicked() {
                                        self.state.playlists.remove(&id);
                                        if self.state.selected_playlist == id {
                                            self.state.selected_playlist = "all".into();
                                        }
                                        if self.playlist_edit.as_ref() == Some(&id) {
                                            self.playlist_edit = None;
                                            self.playlist_name.clear();
                                        }
                                        self.filter();
                                        self.dirty();
                                    }
                                });
                            }
                        });
                    ui.text_edit_singleline(&mut self.playlist_name);
                    ui.horizontal(|ui| {
                        let create = ui.button(l.text("创建", "Create")).clicked();
                        let rename = ui
                            .add_enabled(
                                self.playlist_edit.is_some(),
                                egui::Button::new(l.text("重命名", "Rename")),
                            )
                            .clicked();
                        if create || rename {
                            let id = if rename {
                                self.playlist_edit.as_deref()
                            } else {
                                None
                            };
                            match self.state.name_playlist(id, &self.playlist_name) {
                                Ok(id) => {
                                    self.state.selected_playlist = id;
                                    self.playlist_name.clear();
                                    self.playlist_edit = None;
                                    self.filter();
                                    self.dirty();
                                }
                                Err(error) => self.status = error.to_string(),
                            }
                        }
                    });
                });
            self.playlists = open;
        }
        if self.membership {
            let mut open = true;
            let mut save = false;
            egui::Window::new(l.text("加入 / 移出多个歌单", "Playlist memberships"))
                .open(&mut open)
                .show(ctx, |ui| {
                    if self.state.playlists.is_empty() {
                        ui.label(l.text("请先创建歌单", "Create a playlist first"));
                    }
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for (id, playlist) in &self.state.playlists {
                                let mut included = self.memberships.contains(id);
                                if ui.checkbox(&mut included, &playlist.name).changed() {
                                    if included {
                                        self.memberships.insert(id.clone());
                                    } else {
                                        self.memberships.remove(id);
                                    }
                                }
                            }
                        });
                    save = ui.button(l.text("保存", "Save")).clicked();
                });
            if save && let Some(path) = &self.selected {
                match self.state.memberships(path, &self.memberships) {
                    Ok(()) => {
                        self.filter();
                        self.dirty();
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
            self.membership = open && !save;
        }
        if self.rename {
            let mut open = true;
            let mut save = false;
            egui::Window::new(l.text("显示名称（留空恢复）", "Display name (empty resets)"))
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.text_edit_singleline(&mut self.rename_text);
                    save = ui.button(l.text("保存", "Save")).clicked();
                });
            if save && let Some(path) = &self.selected {
                if self.rename_text.trim().is_empty() {
                    self.state.names.remove(path);
                } else {
                    self.state
                        .names
                        .insert(path.clone(), self.rename_text.trim().into());
                }
                self.filter();
                self.dirty();
            }
            self.rename = open && !save;
        }
        if self.info {
            let mut open = true;
            egui::Window::new(l.text("文件信息", "File information"))
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(track) = self.selected_track() {
                        ui.label(&track.path);
                        ui.label(format!(
                            "{} / {} / {}",
                            track.title, track.artist, track.album
                        ));
                        ui.label(&track.notes);
                        ui.separator();
                        ui.label(l.text("原始标签（未改写）", "Original tags (not modified)"));
                        for tag in &track.raw {
                            ui.label(tag);
                        }
                    }
                });
            self.info = open;
        }
        if self.close_dialog {
            let mut open = true;
            let mut chosen = None;
            egui::Window::new(l.text("关闭行为", "Close behavior"))
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    if self.close_settings {
                        let old = self.state.close_action;
                        for (action, text) in [
                            (CloseAction::Ask, l.text("每次询问", "Ask every time")),
                            (CloseAction::Tray, l.text("最小化托盘", "Minimize to tray")),
                            (CloseAction::Exit, l.text("退出播放器", "Exit player")),
                        ] {
                            ui.radio_value(&mut self.state.close_action, action, text);
                        }
                        if old != self.state.close_action {
                            self.dirty();
                        }
                    } else {
                        ui.checkbox(&mut self.remember, l.text("记住选择", "Remember choice"));
                        if ui
                            .button(
                                l.text("最小化托盘，继续播放", "Minimize to tray, keep playing"),
                            )
                            .clicked()
                        {
                            chosen = Some(CloseAction::Tray);
                        }
                        if ui.button(l.text("退出播放器", "Exit player")).clicked() {
                            chosen = Some(CloseAction::Exit);
                        }
                    }
                });
            self.close_dialog = open;
            if let Some(action) = chosen {
                self.close_dialog = false;
                if action == CloseAction::Tray {
                    self.hide(ctx);
                } else {
                    self.exit(ctx);
                }
                if self.remember && (action == CloseAction::Exit || self.hidden) {
                    self.state.close_action = action;
                    self.dirty();
                }
            }
            if !self.close_dialog {
                self.close_settings = false;
            }
        }
    }
}

impl Desktop {
    /// Drive the real UI lifecycle only when explicitly started with --ui-self-test.
    fn probe_ui(&mut self, ctx: &egui::Context, snapshot: &Snapshot) {
        if let Some(restart) = self.lifecycle {
            if !restart {
                if self.scanning
                    && self.scan_progress.parsed.load(Ordering::Relaxed) >= 16
                    && !self.scan_progress.finished.load(Ordering::Acquire)
                {
                    self.state = crate::diagnostics::lifecycle_state(&std::path::PathBuf::from(
                        &self.state.roots[0],
                    ));
                    self.dirty();
                    self.probe.as_ref().unwrap().store(true, Ordering::Relaxed);
                    self.exit(ctx);
                }
            } else if self.probe_step == 0 && !self.scanning && self.tracks.len() == 1024 {
                // Restore preferences, not an unsolicited playback session.
                if self.visible.len() != 1 || snapshot.current.is_some() {
                    self.exit(ctx);
                    return;
                }
                self.selected = Some(self.tracks[self.visible[0]].path.clone());
                self.play_selected();
                self.probe_step = 1;
            } else if self.probe_step == 1 && snapshot.active && snapshot.duration > 0.0 {
                self.probe.as_ref().unwrap().store(true, Ordering::Relaxed);
                self.exit(ctx);
            }
            return;
        }
        match self.probe_step {
            0 if !self.scanning && self.tracks.len() == 2 => {
                self.query = "歌手 测试".into();
                self.filter();
                if self.visible.len() != 2 {
                    self.exit(ctx);
                    return;
                }
                self.selected = Some(self.tracks[self.visible[0]].path.clone());
                self.play_selected();
                self.probe_step = 1;
            }
            1 if snapshot.active && snapshot.duration > 0.0 => {
                self.command(Command::Toggle);
                self.probe_step = 2;
            }
            2 if snapshot.paused => {
                let first = self
                    .state
                    .name_playlist(None, "测试歌单")
                    .expect("fixture name");
                let second = self
                    .state
                    .name_playlist(None, "Test playlist")
                    .expect("fixture name");
                self.state
                    .memberships(
                        self.selected.as_ref().unwrap(),
                        &BTreeSet::from([first, second]),
                    )
                    .unwrap();
                self.state.language = Language::English;
                self.dirty();
                self.hide(ctx);
                if !self.hidden {
                    self.exit(ctx);
                    return;
                }
                self.probe_step = 3;
                let context = ctx.clone();
                self.runtime.spawn(async move {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    if !post_native_restore() {
                        context.send_viewport_cmd(ViewportCommand::Visible(true));
                    }
                    context.request_repaint();
                });
            }
            3 if !self.hidden && snapshot.paused => {
                self.command(Command::Toggle);
                self.command(Command::Skip(1));
                self.probe_step = 4;
            }
            4 if snapshot.active
                && snapshot
                    .current
                    .as_ref()
                    .is_some_and(|t| t.path == self.tracks[self.visible[1]].path) =>
            {
                self.probe.as_ref().unwrap().store(true, Ordering::Relaxed);
                self.exit(ctx);
                self.probe_step = 5;
            }
            _ => {}
        }
    }
}

/// Test the native notification message, scoped to this process's tray window.
fn post_native_restore() -> bool {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> *mut std::ffi::c_void;
        fn GetWindowThreadProcessId(window: *mut std::ffi::c_void, pid: *mut u32) -> u32;
        fn PostMessageW(
            window: *mut std::ffi::c_void,
            message: u32,
            wparam: usize,
            lparam: isize,
        ) -> i32;
    }
    let class: Vec<_> = "tray_icon_app\0".encode_utf16().collect();
    unsafe {
        let window = FindWindowW(class.as_ptr(), std::ptr::null());
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        !window.is_null() && pid == std::process::id() && PostMessageW(window, 6002, 0, 0x202) != 0
    }
}

// Hidden Windows viewports do not render egui's queued viewport commands.
// Wake the owned HWND first; the next frame processes Restore/Exit normally.
fn wake_window(window: isize) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn ShowWindowAsync(window: *mut std::ffi::c_void, command: i32) -> i32;
    }
    unsafe {
        ShowWindowAsync(window as *mut std::ffi::c_void, 9);
    }
}
