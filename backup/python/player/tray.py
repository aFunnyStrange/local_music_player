"""Windows notification-area icon serviced by the existing main-thread message pump."""
from collections import deque

import win32api
import win32con
import win32gui
import settings

class WindowsTray:
    """Own a hidden Win32 window and icon without a dedicated thread or process."""

    def __init__(self) -> None:
        """Register a private notification receiver on the Tk owner thread."""
        self.events: deque[str] = deque(maxlen=16)
        self.visible = False
        self.closed = False
        self.tooltip = "Shiyin"
        self.callback_message = win32con.WM_APP + 10
        self.taskbar_created = win32gui.RegisterWindowMessage("TaskbarCreated")
        self.instance = win32api.GetModuleHandle(None)
        self.class_name = f"ShiyinTray_{id(self)}"
        window_class = win32gui.WNDCLASS()
        window_class.hInstance = self.instance
        window_class.lpszClassName = self.class_name
        window_class.lpfnWndProc = self._window_proc
        win32gui.RegisterClass(window_class)
        try:
            self.hwnd = win32gui.CreateWindow(self.class_name, "拾音托盘消息窗口", 0,
                                             0, 0, 0, 0, 0, 0, self.instance, None)
            self.owns_icon = settings.ICON_FILE.is_file()
            if self.owns_icon:
                self.icon = win32gui.LoadImage(0, str(settings.ICON_FILE), win32con.IMAGE_ICON,
                                              win32api.GetSystemMetrics(win32con.SM_CXSMICON),
                                              win32api.GetSystemMetrics(win32con.SM_CYSMICON),
                                              win32con.LR_LOADFROMFILE)
            else:
                self.icon = win32gui.LoadIcon(0, win32con.IDI_APPLICATION)
        except Exception:
            if hasattr(self, "hwnd"):
                win32gui.DestroyWindow(self.hwnd)
            win32gui.UnregisterClass(self.class_name, self.instance)
            raise

    def _window_proc(self, hwnd: int, message: int, wparam: int, lparam: int) -> int:
        """Queue native events; never enter Tk recursively from a Win32 callback."""
        if message == self.taskbar_created and self.visible:
            self.events.append("recreate")
            return 0
        if message == self.callback_message:
            if lparam in (win32con.WM_LBUTTONUP, win32con.WM_LBUTTONDBLCLK):
                self.events.append("restore")
            elif lparam == win32con.WM_RBUTTONUP:
                self.events.append("menu")
            return 0
        return win32gui.DefWindowProc(hwnd, message, wparam, lparam)

    def show(self) -> None:
        """Add the icon before the caller hides its window; propagate shell failures."""
        if self.closed:
            raise RuntimeError("托盘已经关闭")
        if not self.visible:
            flags = win32gui.NIF_ICON | win32gui.NIF_MESSAGE | win32gui.NIF_TIP
            win32gui.Shell_NotifyIcon(win32gui.NIM_ADD,
                                     (self.hwnd, 1, flags, self.callback_message, self.icon,
                                      self.tooltip))
            self.visible = True

    def set_tooltip(self, text: str) -> None:
        """Update translated tooltip without replacing the notification icon."""
        self.tooltip = text
        if self.visible:
            win32gui.Shell_NotifyIcon(win32gui.NIM_MODIFY,
                                     (self.hwnd, 1, win32gui.NIF_TIP, self.callback_message,
                                      self.icon, self.tooltip))

    def hide(self) -> None:
        """Remove the icon after restoration or during final shutdown."""
        if self.visible:
            try:
                win32gui.Shell_NotifyIcon(win32gui.NIM_DELETE, (self.hwnd, 1))
            finally:
                self.visible = False

    def recreate(self) -> None:
        """Restore the notification icon when Explorer rebuilds the taskbar."""
        try:
            win32gui.Shell_NotifyIcon(win32gui.NIM_DELETE, (self.hwnd, 1))
        except win32gui.error:
            pass  # Explorer may already have discarded the old registration.
        self.visible = False
        self.show()

    def close(self) -> None:
        """Release owned window/class resources without posting a process-wide quit."""
        if self.closed:
            return
        try:
            self.hide()
        finally:
            self.closed = True
            win32gui.DestroyWindow(self.hwnd)
            win32gui.UnregisterClass(self.class_name, self.instance)
            self.events.clear()
            if self.owns_icon:
                win32gui.DestroyIcon(self.icon)
