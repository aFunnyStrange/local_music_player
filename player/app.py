"""Run Tk and asyncio on the main thread with bounded blocking I/O workers."""
import asyncio
import logging
import sys
import tkinter as tk
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from tkinter import filedialog, messagebox, simpledialog, ttk
from typing import Coroutine, Optional

import settings
from .library import Track, load_state, path_key, save_state, scan_library, visible_tracks
from .playback import MODES, Playback, VlcBackend
from .tray import WindowsTray

log = logging.getLogger(__name__)


def clock_text(seconds: float) -> str:
    """Render nonnegative playback time."""
    minutes, seconds = divmod(int(max(0, seconds)), 60)
    return f"{minutes:02d}:{seconds:02d}"


class Application:
    """Own all widgets and tasks; background workers never access Tk."""

    def __init__(self, root: tk.Tk, state: dict, playback: Playback,
                 state_file: Path = settings.STATE_FILE) -> None:
        """Build a single resizable window and retain explicit runtime ownership."""
        self.root, self.state, self.playback = root, state, playback
        self.state_file = state_file
        self.tracks: list[Track] = []
        self.visible: list[Track] = []
        self.tasks: set[asyncio.Task] = set()
        self.scan_task: Optional[asyncio.Task] = None
        self.render_task: Optional[asyncio.Task] = None
        self.save_task: Optional[asyncio.Task] = None
        self.search_timer = None
        self.closing = False
        self.tray: Optional[WindowsTray] = None
        self.hidden_to_tray = False
        self.close_dialog: Optional[tk.Toplevel] = None
        self.tray_popup: Optional[tk.Toplevel] = None
        self.hidden_children: list[tk.Toplevel] = []
        self.scan_errors: list[str] = []
        self.save_lock = asyncio.Lock()
        self.save_dirty = False
        self.favorites = set(state["favorites"])
        self.query = tk.StringVar()
        self.only_favorites = tk.BooleanVar()
        self.status = tk.StringVar(value="添加音乐文件夹，或把文件放入 library 目录")
        self.now = tk.StringVar(value="还没有播放歌曲")
        self.time_text = tk.StringVar(value="00:00 / 00:00")
        self.mode = tk.StringVar(value=state["mode"])
        self.volume = tk.DoubleVar(value=state["volume"])
        self.progress = tk.DoubleVar()
        self.dragging = False
        playback.mode, playback.volume = state["mode"], int(state["volume"])
        self._build()
        self.root.protocol("WM_DELETE_WINDOW", self.on_window_close)
        self.tray_menu = tk.Menu(root, tearoff=False)
        self.tray_menu.add_command(label="打开播放器", command=self.restore_window)
        self.tray_menu.add_command(label="播放 / 暂停", command=self.toggle_play)
        self.tray_menu.add_command(label="上一首", command=lambda: self.launch(self.playback.skip(-1)))
        self.tray_menu.add_command(label="下一首", command=lambda: self.launch(self.playback.skip(1)))
        self.tray_menu.add_separator()
        self.tray_menu.add_command(label="退出播放器", command=self.request_close)

    def _build(self) -> None:
        """Compose a searchable library, real scrollbars and stable playback controls."""
        root = self.root
        root.title("拾音 · 本地音乐播放器")
        root.geometry("1100x720")
        root.minsize(800, 480)
        style = ttk.Style(root)
        style.configure("Treeview", rowheight=29)
        style.configure("TButton", padding=(10, 6))
        frame = ttk.Frame(root, padding=16)
        frame.pack(fill="both", expand=True)
        toolbar = ttk.Frame(frame)
        toolbar.pack(fill="x", pady=(0, 12))
        ttk.Label(toolbar, text="拾音", font=("Microsoft YaHei UI", 20, "bold")).pack(side="left", padx=(0, 24))
        ttk.Button(toolbar, text="添加文件夹", command=self.add_folder).pack(side="left")
        ttk.Button(toolbar, text="管理目录", command=self.manage_folders).pack(side="left", padx=6)
        ttk.Button(toolbar, text="刷新", command=self.refresh).pack(side="left")
        ttk.Button(toolbar, text="取消扫描", command=self.cancel_scan).pack(side="left", padx=6)
        ttk.Checkbutton(toolbar, text="只看收藏", variable=self.only_favorites,
                        command=self.render).pack(side="right")
        search = ttk.Frame(frame)
        search.pack(fill="x", pady=(0, 12))
        ttk.Label(search, text="搜索歌曲 / 歌手 / 专辑 / 文件名").pack(side="left", padx=(0, 12))
        self.search_entry = ttk.Entry(search, textvariable=self.query)
        self.search_entry.pack(side="left", fill="x", expand=True)
        ttk.Button(search, text="清空", command=lambda: self.query.set("")).pack(side="left", padx=(8, 0))
        self.query.trace_add("write", self.search_changed)
        body = ttk.Frame(frame)
        body.pack(fill="both", expand=True)
        columns = ("favorite", "title", "artist", "album", "duration", "file")
        self.tree = ttk.Treeview(body, columns=columns, show="headings", selectmode="browse")
        for column, label, width in zip(columns, ("收藏", "歌曲", "歌手", "专辑", "时长", "原文件名"),
                                         (50, 260, 140, 150, 65, 260)):
            self.tree.heading(column, text=label)
            self.tree.column(column, width=width, minwidth=40, stretch=column not in ("favorite", "duration"))
        vertical = ttk.Scrollbar(body, orient="vertical", command=self.tree.yview)
        horizontal = ttk.Scrollbar(body, orient="horizontal", command=self.tree.xview)
        self.tree.configure(yscrollcommand=vertical.set, xscrollcommand=horizontal.set)
        self.tree.grid(row=0, column=0, sticky="nsew")
        vertical.grid(row=0, column=1, sticky="ns")
        horizontal.grid(row=1, column=0, sticky="ew")
        body.rowconfigure(0, weight=1)
        body.columnconfigure(0, weight=1)
        self.tree.bind("<Double-1>", self.play_selected)
        self.tree.bind("<Return>", self.play_selected)
        actions = ttk.Frame(frame)
        actions.pack(fill="x", pady=8)
        ttk.Button(actions, text="收藏 / 取消收藏", command=self.toggle_favorite).pack(side="left")
        ttk.Button(actions, text="修改显示名称", command=self.rename_display).pack(side="left", padx=6)
        ttk.Button(actions, text="文件信息", command=self.file_info).pack(side="left")
        ttk.Button(actions, text="关闭设置", command=lambda: self.show_close_dialog(True)).pack(side="right")
        ttk.Label(frame, textvariable=self.now, font=("Microsoft YaHei UI", 12)).pack(anchor="w", pady=(8, 4))
        timeline = ttk.Frame(frame)
        timeline.pack(fill="x")
        self.slider = ttk.Scale(timeline, from_=0, to=1, variable=self.progress)
        self.slider.pack(side="left", fill="x", expand=True)
        self.slider.bind("<ButtonPress-1>", self.begin_seek)
        self.slider.bind("<ButtonRelease-1>", self.end_seek)
        ttk.Label(timeline, textvariable=self.time_text, width=17, anchor="e").pack(side="right")
        controls = ttk.Frame(frame)
        controls.pack(fill="x", pady=8)
        ttk.Button(controls, text="上一首", command=lambda: self.launch(self.playback.skip(-1))).pack(side="left")
        self.pause_button = ttk.Button(controls, text="播放", command=self.toggle_play)
        self.pause_button.pack(side="left", padx=6)
        ttk.Button(controls, text="下一首", command=lambda: self.launch(self.playback.skip(1))).pack(side="left")
        modes = ttk.Combobox(controls, values=MODES, state="readonly", textvariable=self.mode, width=12)
        modes.pack(side="left", padx=12)
        modes.bind("<<ComboboxSelected>>", self.change_mode)
        volume = ttk.Scale(controls, from_=0, to=100, variable=self.volume)
        volume.pack(side="right")
        volume.bind("<ButtonRelease-1>", self.change_volume)
        volume.bind("<KeyRelease>", self.change_volume)
        ttk.Label(controls, text="音量").pack(side="right", padx=8)
        ttk.Label(frame, textvariable=self.status, wraplength=1000).pack(anchor="w")
        root.bind("<Control-f>", self.focus_search)
        root.bind("<space>", self.space_play)

    def launch(self, coroutine: Coroutine) -> Optional[asyncio.Task]:
        """Retain a bounded number of commands and consume every task exception."""
        if self.closing or len(self.tasks) >= 32:
            coroutine.close()
            return None
        task = asyncio.create_task(coroutine)
        self.tasks.add(task)
        task.add_done_callback(self.task_done)
        return task

    def task_done(self, task: asyncio.Task) -> None:
        """Route failures back to the main-thread status area."""
        self.tasks.discard(task)
        if task.cancelled():
            return
        error = task.exception()
        if error:
            log.error("Background task failed", exc_info=(type(error), error, error.__traceback__))
            if not self.closing:
                self.status.set(f"操作失败：{error}")

    def selected(self) -> Optional[Track]:
        """Resolve a row by file identity, independent of sorting and filtering."""
        selection = self.tree.selection()
        return next((track for track in self.visible if track.path == selection[0]), None) if selection else None

    def play_selected(self, event=None) -> None:
        """Play the selected row and snapshot the currently filtered playlist."""
        track = self.selected()
        if track:
            self.launch(self.playback.play(self.visible, self.visible.index(track)))

    def toggle_play(self) -> None:
        """Start the selected item or toggle the active stream."""
        if self.playback.current is None:
            self.play_selected()
        else:
            self.launch(self.playback.toggle_pause())

    def space_play(self, event) -> Optional[str]:
        """Keep typing spaces in text fields; otherwise use the playback shortcut."""
        if event.widget.winfo_class() not in ("Entry", "TEntry", "TCombobox"):
            self.toggle_play()
            return "break"
        return None

    def focus_search(self, event=None) -> str:
        """Focus search without changing playback."""
        self.search_entry.focus_set()
        return "break"

    def search_changed(self, *args) -> None:
        """Debounce typing to avoid rebuilding the table on every keypress."""
        if self.search_timer:
            self.root.after_cancel(self.search_timer)
        self.search_timer = self.root.after(150, self.render)

    def render(self) -> None:
        """Replace an obsolete rendering task and rebuild rows cooperatively."""
        self.search_timer = None
        if self.render_task:
            self.render_task.cancel()
        self.render_task = self.launch(self.render_rows())

    async def render_rows(self) -> None:
        """Yield between row batches so a large library keeps accepting UI events."""
        selection = self.tree.selection()
        self.visible = visible_tracks(self.tracks, self.query.get(), self.favorites,
                                      self.state["names"], self.only_favorites.get())
        children = self.tree.get_children()
        for offset in range(0, len(children), 200):
            self.tree.delete(*children[offset:offset + 200])
            await asyncio.sleep(0)
        for index, track in enumerate(self.visible):
            self.tree.insert("", "end", iid=track.path, values=(
                "♥" if track.path in self.favorites else "",
                self.state["names"].get(track.path, track.title), track.artist, track.album,
                clock_text(track.duration), Path(track.path).name))
            if index % 100 == 99:
                await asyncio.sleep(0)
        if selection and self.tree.exists(selection[0]):
            self.tree.selection_set(selection[0])
        self.status.set(f"显示 {len(self.visible)} / {len(self.tracks)} 首"
                        + (f"；有 {len(self.scan_errors)} 个目录读取失败" if self.scan_errors else ""))

    def refresh(self) -> None:
        """Keep at most one library scan active."""
        if self.scan_task and not self.scan_task.done():
            self.status.set("正在扫描；可点击取消扫描")
            return
        self.scan_task = self.launch(self.scan())

    async def scan(self) -> None:
        """Atomically replace the library on completion; cancellation preserves old results."""
        found, errors = {}, []
        try:
            async for batch in scan_library(list(self.state["roots"]), settings.AUDIO_EXTENSIONS,
                                            settings.SCAN_BATCH_SIZE, errors):
                found.update((track.path, track) for track in batch)
                self.status.set(f"正在读取音乐信息：{len(found)} 首")
            self.tracks = list(found.values())
            self.scan_errors = errors
            self.render()
        except asyncio.CancelledError:
            if not self.closing:
                self.status.set("扫描已取消，保留上次列表")
            raise

    def cancel_scan(self) -> None:
        """Cancel discovery without touching audio files or playback."""
        if self.scan_task:
            self.scan_task.cancel()

    def add_folder(self) -> None:
        """Index an external directory without copying or moving its files."""
        folder = filedialog.askdirectory(parent=self.root, title="选择音乐文件夹（不会移动文件）")
        if folder:
            self.launch(self.add_root(folder))

    async def add_root(self, folder: str) -> None:
        """Normalize a selected path outside the UI thread before persisting it."""
        key = await asyncio.to_thread(path_key, Path(folder))
        if key not in self.state["roots"]:
            self.state["roots"].append(key)
            self.schedule_save()
        if self.scan_task and not self.scan_task.done():
            self.scan_task.cancel()
            await asyncio.gather(self.scan_task, return_exceptions=True)
        self.refresh()

    def manage_folders(self) -> None:
        """Remove indexing roots without deleting any user audio."""
        dialog = tk.Toplevel(self.root)
        dialog.title("音乐目录（移除目录不会删除音乐）")
        dialog.geometry("700x300")
        listing = tk.Listbox(dialog, exportselection=False)
        listing.pack(fill="both", expand=True, padx=12, pady=12)
        for folder in self.state["roots"]:
            listing.insert("end", folder)

        def remove() -> None:
            """Remove only the selected configured root."""
            selected = listing.curselection()
            if selected:
                folder = listing.get(selected[0])
                self.state["roots"].remove(folder)
                listing.delete(selected[0])
                self.schedule_save()
                self.launch(self.rescan())

        ttk.Button(dialog, text="移除选中目录", command=remove).pack(pady=(0, 12))

    async def rescan(self) -> None:
        """Restart a scan after directory configuration changes."""
        if self.scan_task and not self.scan_task.done():
            self.scan_task.cancel()
            await asyncio.gather(self.scan_task, return_exceptions=True)
        self.refresh()

    def toggle_favorite(self) -> None:
        """Persist a virtual favorite without moving or renaming files."""
        track = self.selected()
        if track:
            self.favorites.symmetric_difference_update({track.path})
            self.schedule_save()
            self.render()

    def rename_display(self) -> None:
        """Offer a reversible display override for bad tags or garbled filenames."""
        track = self.selected()
        if not track:
            return
        name = simpledialog.askstring("显示名称", "只修改本播放器中的名称；留空恢复自动识别：",
                                      initialvalue=self.state["names"].get(track.path, track.title), parent=self.root)
        if name is not None:
            if name.strip():
                self.state["names"][track.path] = name.strip()
            else:
                self.state["names"].pop(track.path, None)
            self.schedule_save()
            self.render()

    def file_info(self) -> None:
        """Show source identity and metadata without modifying the audio."""
        track = self.selected()
        if track:
            messagebox.showinfo("音乐文件", f"{track.path}\n\n显示标题：{track.title}\n歌手：{track.artist}\n专辑：{track.album}"
                                f"\n\n识别依据\n{track.display_note}"
                                f"\n\n原始标签（未修改）\n标题：{track.raw_title}\n歌手：{track.raw_artist}\n专辑：{track.raw_album}",
                                parent=self.root)

    def change_mode(self, event=None) -> None:
        """Apply mode changes to the next transition without restarting playback."""
        self.playback.mode = self.mode.get()
        self.schedule_save()

    def change_volume(self, event=None) -> None:
        """Commit slider changes on release rather than queueing every pixel."""
        self.launch(self.playback.set_volume(round(self.volume.get())))
        self.schedule_save()

    def begin_seek(self, event=None) -> None:
        """Temporarily stop progress updates while the user drags."""
        self.dragging = True

    def end_seek(self, event=None) -> None:
        """Seek to the user-selected position and resume progress updates."""
        self.dragging = False
        self.launch(self.playback.seek(self.progress.get()))

    def snapshot_state(self) -> dict:
        """Create a detached snapshot so worker writes cannot race with UI edits."""
        return {**self.state, "roots": list(self.state["roots"]),
                "favorites": sorted(self.favorites), "names": dict(self.state["names"]),
                "volume": round(self.volume.get()), "mode": self.mode.get()}

    def schedule_save(self) -> None:
        """Coalesce edits into one delayed writer; closing performs a final flush."""
        self.save_dirty = True
        if not self.save_task or self.save_task.done():
            self.save_task = self.launch(self.delayed_save())

    async def delayed_save(self) -> None:
        """Save the latest state after a short burst of UI edits."""
        while self.save_dirty:
            await asyncio.sleep(0.4)
            self.save_dirty = False
            await self.save()

    async def save(self) -> None:
        """Serialize atomic filesystem writes."""
        async with self.save_lock:
            await asyncio.to_thread(save_state, self.state_file, self.snapshot_state())

    async def monitor(self) -> None:
        """Poll one decoder at modest frequency and update widgets on the main thread."""
        while not self.closing:
            try:
                await self.playback.tick()
            except Exception as exc:
                log.exception("Playback monitor failed")
                self.status.set(f"播放失败：{exc}")
            track = self.playback.current
            if track:
                prefix = "已暂停" if self.playback.paused else ("正在播放" if self.playback.active else "已停止")
                self.now.set(f"{prefix} · {self.state['names'].get(track.path, track.title)}  {track.artist}")
            self.pause_button.configure(text=self.transport_label())
            self.slider.configure(to=max(1, self.playback.duration))
            if not self.dragging:
                self.progress.set(self.playback.position)
            self.time_text.set(f"{clock_text(self.playback.position)} / {clock_text(self.playback.duration)}")
            await asyncio.sleep(0.2)

    def on_window_close(self) -> None:
        """Apply the saved X-button policy, leaving explicit Exit unconditional."""
        if self.closing:
            return
        action = self.state.get("close_action", "ask")
        if action == "tray":
            self.minimize_to_tray()
        elif action == "exit":
            self.request_close()
        else:
            self.show_close_dialog()

    def show_close_dialog(self, settings_only: bool = False) -> None:
        """Ask without wait_window so playback and asyncio keep running."""
        if self.close_dialog and self.close_dialog.winfo_exists():
            self.close_dialog.lift()
            return
        dialog = self.close_dialog = tk.Toplevel(self.root)
        dialog.title("关闭设置" if settings_only else "关闭播放器")
        dialog.transient(self.root)
        dialog.resizable(False, False)
        body = ttk.Frame(dialog, padding=20)
        body.pack(fill="both", expand=True)
        ttk.Label(body, text="点击右上角 X 时：" if settings_only else "你希望如何关闭播放器？").pack(anchor="w", pady=(0, 12))
        initial = self.state.get("close_action", "ask")
        action = tk.StringVar(value=initial if settings_only or initial != "ask" else "tray")
        remember = tk.BooleanVar(value=False)
        if settings_only:
            ttk.Radiobutton(body, text="每次询问", value="ask", variable=action).pack(anchor="w", pady=4)
        ttk.Radiobutton(body, text="最小化到系统托盘，继续播放", value="tray", variable=action).pack(anchor="w", pady=4)
        ttk.Radiobutton(body, text="退出播放器，停止播放", value="exit", variable=action).pack(anchor="w", pady=4)
        if not settings_only:
            ttk.Checkbutton(body, text="记住选择（可在关闭设置中修改）", variable=remember).pack(anchor="w", pady=12)

        def dismiss() -> None:
            """Cancel without changing playback or persisted preferences."""
            dialog.grab_release()
            dialog.destroy()
            self.close_dialog = None

        def confirm() -> None:
            """Persist only accepted actions and keep a failed tray operation recoverable."""
            choice, persist = action.get(), remember.get()
            dismiss()
            if settings_only:
                self.state["close_action"] = choice
                self.schedule_save()
            else:
                self.apply_close_choice(choice, persist)

        buttons = ttk.Frame(body)
        buttons.pack(fill="x", pady=(12, 0))
        ttk.Button(buttons, text="取消", command=dismiss).pack(side="right")
        ttk.Button(buttons, text="保存" if settings_only else "确定", command=confirm).pack(side="right", padx=8)
        dialog.protocol("WM_DELETE_WINDOW", dismiss)
        dialog.bind("<Escape>", lambda event: dismiss())
        dialog.grab_set()
        dialog.focus_set()

    def apply_close_choice(self, choice: str, remember: bool) -> None:
        """Execute the chosen close action without confusing tray hiding with shutdown."""
        if choice == "tray" and not self.minimize_to_tray():
            return
        if remember:
            self.state["close_action"] = choice
            self.schedule_save()
        if choice == "exit":
            self.request_close()

    def minimize_to_tray(self) -> bool:
        """Hide only after Windows has accepted the tray icon."""
        self.dismiss_tray_menu()
        try:
            if self.tray is None:
                self.tray = WindowsTray()
            self.tray.show()
        except Exception as exc:
            log.exception("Cannot create notification icon")
            messagebox.showerror("无法最小化到托盘", f"{exc}\n窗口将保持打开，你仍可选择退出。", parent=self.root)
            return False
        if not self.hidden_to_tray:
            self.hidden_children = [child for child in self.root.winfo_children()
                                    if isinstance(child, tk.Toplevel) and child.winfo_viewable()]
            for child in self.hidden_children:
                child.withdraw()
            self.hidden_to_tray = True
            self.root.withdraw()
        return True

    def restore_window(self) -> None:
        """Restore the player before removing its alternative access point."""
        self.dismiss_tray_menu()
        self.root.deiconify()
        self.root.lift()
        self.root.focus_force()
        self.hidden_to_tray = False
        for child in self.hidden_children:
            if child.winfo_exists():
                child.deiconify()
        self.hidden_children.clear()
        if self.tray:
            self.tray.hide()

    def process_tray_events(self) -> None:
        """Drain native notifications between Tk updates on the main asyncio thread."""
        if not self.tray:
            return
        while self.tray.events and not self.closing:
            event = self.tray.events.popleft()
            if event == "restore":
                self.restore_window()
            elif event == "menu":
                self.show_tray_menu()
            elif event == "recreate":
                try:
                    self.tray.recreate()
                except Exception:
                    log.exception("Cannot restore tray after Explorer restart")
                    self.restore_window()
                    self.status.set("系统托盘恢复失败，已重新显示播放器")

    def update_tray_menu(self) -> None:
        """Describe the current transport state when opening the tray menu."""
        label = self.transport_label()
        can_play = self.playback.current is not None or self.selected() is not None
        self.tray_menu.entryconfigure(1, label=label, state="normal" if can_play else "disabled")
        for index, step in ((2, -1), (3, 1)):
            available = self.playback.next_index(step) is not None
            self.tray_menu.entryconfigure(index, state="normal" if available else "disabled")

    def transport_label(self) -> str:
        """Use one consistent play/pause/resume action in both surfaces."""
        return "继续" if self.playback.paused else ("暂停" if self.playback.active else "播放")

    def dismiss_tray_menu(self) -> None:
        """Close the nonmodal popup without affecting the hidden player."""
        if self.tray_popup:
            self.tray_popup.destroy()
            self.tray_popup = None

    def show_tray_menu(self) -> None:
        """Display a nonmodal Tk popup instead of entering Windows' blocking menu loop."""
        self.dismiss_tray_menu()
        self.update_tray_menu()
        popup = self.tray_popup = tk.Toplevel(self.root)
        popup.withdraw()
        popup.overrideredirect(True)
        popup.attributes("-topmost", True)
        frame = ttk.Frame(popup, padding=5, relief="solid", borderwidth=1)
        frame.pack(fill="both", expand=True)

        def select(index: int) -> None:
            """Dismiss before invoking a transport action on the main thread."""
            self.dismiss_tray_menu()
            self.tray_menu.invoke(index)

        def dismiss_if_unfocused() -> None:
            """Keep focus moves between popup children from closing the menu."""
            if self.tray_popup is not popup:
                return
            focused = self.root.focus_get()
            if focused is None or focused.winfo_toplevel() is not popup:
                self.dismiss_tray_menu()

        for index in range(self.tray_menu.index("end") + 1):
            if self.tray_menu.type(index) == "separator":
                ttk.Separator(frame).pack(fill="x", pady=4)
            else:
                ttk.Button(frame, text=self.tray_menu.entrycget(index, "label"), width=20,
                           state=self.tray_menu.entrycget(index, "state"),
                           command=lambda index=index: select(index)).pack(fill="x")
        popup.bind("<Escape>", lambda event: self.dismiss_tray_menu())
        popup.bind("<FocusOut>", lambda event: popup.after_idle(dismiss_if_unfocused))
        popup.update_idletasks()
        x = max(0, min(self.root.winfo_pointerx(), self.root.winfo_screenwidth() - popup.winfo_reqwidth()))
        y = max(0, min(self.root.winfo_pointery(), self.root.winfo_screenheight() - popup.winfo_reqheight()))
        popup.geometry(f"+{x}+{y}")
        popup.deiconify()
        popup.focus_force()

    def request_close(self) -> None:
        """Signal graceful async shutdown rather than destroying widgets mid-task."""
        self.dismiss_tray_menu()
        self.closing = True

    async def run(self) -> None:
        """Pump Tk from asyncio; one main thread owns all application state."""
        self.refresh()
        monitor = self.launch(self.monitor())
        try:
            while not self.closing:
                self.root.update()
                self.process_tray_events()
                await asyncio.sleep(0.05 if self.hidden_to_tray else 0.016)
        finally:
            self.closing = True
            if self.tray:
                try:
                    self.tray.close()
                except Exception:
                    log.exception("Cannot fully release tray resources")
            # Cancel read-only producers, but let decoder commands and writes finish.
            for task in (self.scan_task, self.render_task, monitor):
                if task:
                    task.cancel()
            await asyncio.gather(*list(self.tasks), return_exceptions=True)
            try:
                await self.save()
            except Exception as exc:
                log.exception("Cannot save library")
                self.root.deiconify()
                messagebox.showerror("配置保存失败", str(exc), parent=self.root)
            finally:
                try:
                    await self.playback.close()
                finally:
                    self.root.destroy()


async def main() -> None:
    """Construct the application and its sole shared blocking pool."""
    if sys.platform != "win32":
        raise RuntimeError("拾音仅支持 Windows；不支持 Linux 或 macOS。")
    loop = asyncio.get_running_loop()
    pool = ThreadPoolExecutor(max_workers=settings.IO_WORKERS, thread_name_prefix="music-io")
    loop.set_default_executor(pool)
    root = tk.Tk()
    root.withdraw()
    backend = None
    try:
        state = await asyncio.to_thread(load_state, settings.STATE_FILE, settings.LIBRARY_DIR)
        await asyncio.to_thread(settings.LIBRARY_DIR.mkdir, parents=True, exist_ok=True)
        backend = await asyncio.to_thread(VlcBackend, settings.VLC_DIR)
    except Exception as exc:
        log.exception("Player initialization failed")
        messagebox.showerror("播放器启动失败",
                            f"{exc}\n\n请安装 requirements.txt 中的依赖及同架构 VLC 3，"
                            "或将便携 VLC 放入 .runtime/vlc-3.0.21。配置损坏时请先备份 data/library.json。",
                            parent=root)
        root.destroy()
        return
    root.deiconify()
    app = Application(root, state, Playback(backend))
    await app.run()

