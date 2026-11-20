use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Accessibility::{
    AccessibleObjectFromWindow, SetWinEventHook, HWINEVENTHOOK, IAccessible,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, IsIconic, EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, GetClassNameW, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo,
    GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, GUITHREADINFO, GWL_EXSTYLE, HWND_TOPMOST,
    OBJID_CARET, SET_WINDOW_POS_FLAGS, SWP_FRAMECHANGED, SWP_HIDEWINDOW, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_SHOW, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

const CARET_LOOKUP_TIMEOUT_MS: u64 = 200;
const CURSOR_HEIGHT_LOGICAL: f64 = 20.0;
const ACTIVATE_POLL_MS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    fn from_win(rect: RECT) -> Self {
        Self { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom }
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub rect: Rect,
    pub from_caret: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    pub work: Rect,
    pub scale: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub above: bool,
}

pub fn place(
    anchor: Rect,
    work: Rect,
    content: (i32, i32),
    inset: i32,
    gap: i32,
    prefer_above: Option<bool>,
) -> Placement {
    let (cw, ch) = (content.0.max(1), content.1.max(1));
    let below_top = anchor.bottom + gap;
    let above_top = anchor.top - gap - ch;
    let fits_below = below_top + ch <= work.bottom;
    let fits_above = above_top >= work.top;
    let above = match prefer_above {
        Some(true) if fits_above => true,
        Some(false) if fits_below => false,
        _ if fits_below => false,
        _ if fits_above => true,
        _ => anchor.top - work.top > work.bottom - anchor.bottom,
    };
    let wanted_top = if above { above_top } else { below_top };
    let top = wanted_top.min(work.bottom - ch).max(work.top);
    let left = anchor.left.min(work.right - cw).max(work.left);
    Placement {
        x: left - inset,
        y: top - inset,
        width: cw + 2 * inset,
        height: ch + 2 * inset,
        above,
    }
}

pub fn to_physical(logical: f64, scale: f64) -> i32 {
    (logical * scale).round() as i32
}

pub fn is_shell_class(class: &str) -> bool {
    matches!(
        class,
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
            | "Progman"
            | "WorkerW"
            | "Windows.UI.Core.CoreWindow"
    )
}

pub fn overlay_ex_style(current: u32) -> u32 {
    (current | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) & !WS_EX_APPWINDOW.0
}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut c_void)
}

pub fn foreground() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

fn class_name(window: HWND) -> String {
    let mut buf = [0u16; 128];
    let len = unsafe { GetClassNameW(window, &mut buf) };
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

pub fn is_own_or_shell(raw: isize) -> bool {
    let window = hwnd(raw);
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    pid == unsafe { GetCurrentProcessId() } || is_shell_class(&class_name(window))
}

static LAST_APP_WINDOW: AtomicIsize = AtomicIsize::new(0);

fn remember_app_window(raw: isize) {
    if raw != 0 && !is_own_or_shell(raw) {
        LAST_APP_WINDOW.store(raw, Ordering::SeqCst);
    }
}

unsafe extern "system" fn on_foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    window: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    remember_app_window(window.0 as isize);
}

pub fn track_foreground() -> bool {
    remember_app_window(foreground());
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(on_foreground_changed),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    !hook.is_invalid()
}

pub fn last_app_window() -> isize {
    LAST_APP_WINDOW.load(Ordering::SeqCst)
}

pub fn activate(raw: isize, timeout_ms: u64) -> bool {
    if raw == 0 || !unsafe { IsWindow(Some(hwnd(raw))) }.as_bool() {
        return false;
    }
    if unsafe { IsIconic(hwnd(raw)) }.as_bool() {
        return false;
    }
    if foreground() == raw {
        return true;
    }
    let _ = unsafe { SetForegroundWindow(hwnd(raw)) };
    let start = Instant::now();
    loop {
        if foreground() == raw {
            return true;
        }
        if start.elapsed() >= Duration::from_millis(timeout_ms) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(ACTIVATE_POLL_MS));
    }
}

pub fn force_foreground(raw: isize) -> bool {
    let target = hwnd(raw);
    unsafe {
        if SetForegroundWindow(target).as_bool() && foreground() == raw {
            return true;
        }
        let current = GetCurrentThreadId();
        let owner = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let attached = owner != 0 && owner != current && AttachThreadInput(current, owner, true).as_bool();
        let _ = BringWindowToTop(target);
        let _ = SetForegroundWindow(target);
        if attached {
            let _ = AttachThreadInput(current, owner, false);
        }
    }
    foreground() == raw
}

pub fn palette_ex_style(current: u32) -> u32 {
    (current | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) & !(WS_EX_APPWINDOW.0 | WS_EX_NOACTIVATE.0)
}

pub fn ensure_overlay_style(raw: isize) {
    apply_ex_style(raw, overlay_ex_style);
}

fn apply_ex_style(raw: isize, style: fn(u32) -> u32) {
    let window = hwnd(raw);
    unsafe {
        let current = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        let wanted = style(current);
        if wanted != current {
            SetWindowLongPtrW(window, GWL_EXSTYLE, wanted as isize);
            let _ = SetWindowPos(
                window,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

pub fn show_without_activating(raw: isize, placement: Placement) {
    ensure_overlay_style(raw);
    position(raw, placement, SWP_NOACTIVATE | SWP_SHOWWINDOW);
}

pub fn move_without_activating(raw: isize, placement: Placement) {
    position(raw, placement, SWP_NOACTIVATE);
}

pub fn show_and_activate(raw: isize) {
    apply_ex_style(raw, palette_ex_style);
    unsafe {
        let _ = ShowWindow(hwnd(raw), SW_SHOW);
    }
}

fn position(raw: isize, placement: Placement, flags: SET_WINDOW_POS_FLAGS) {
    let window = hwnd(raw);
    let set = || unsafe {
        let _ = SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            placement.x,
            placement.y,
            placement.width,
            placement.height,
            flags,
        );
    };
    set();
    let mut actual = RECT::default();
    if unsafe { GetWindowRect(window, &mut actual) }.is_ok() {
        let wanted = Rect {
            left: placement.x,
            top: placement.y,
            right: placement.x + placement.width,
            bottom: placement.y + placement.height,
        };
        if Rect::from_win(actual) != wanted {
            set();
        }
    }
}

pub fn hide_without_activating(raw: isize) {
    unsafe {
        let _ = SetWindowPos(
            hwnd(raw),
            None,
            0,
            0,
            0,
            0,
            SWP_HIDEWINDOW | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

pub fn screen_for(rect: Rect) -> Screen {
    let win_rect = RECT { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom };
    unsafe {
        let monitor = MonitorFromRect(&win_rect, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let work = if GetMonitorInfoW(monitor, &mut info).as_bool() {
            Rect::from_win(info.rcWork)
        } else {
            Rect { left: 0, top: 0, right: 1920, bottom: 1080 }
        };
        let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
        let scale = if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
            dpi_x.max(96) as f64 / 96.0
        } else {
            1.0
        };
        Screen { work, scale }
    }
}

fn window_rect(window: HWND) -> Option<Rect> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(window, &mut rect) }.ok()?;
    Some(Rect::from_win(rect))
}

fn plausible(caret: Rect, host: Option<Rect>) -> bool {
    if caret.bottom <= caret.top {
        return false;
    }
    match host {
        Some(host) => host.contains_point(caret.left, caret.top),
        None => true,
    }
}

fn gui_thread_caret(foreground: HWND) -> (Option<Rect>, HWND) {
    unsafe {
        let thread = GetWindowThreadProcessId(foreground, None);
        let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        if thread == 0 || GetGUIThreadInfo(thread, &mut info).is_err() {
            return (None, foreground);
        }
        let focus = if info.hwndFocus.is_invalid() { foreground } else { info.hwndFocus };
        if info.hwndCaret.is_invalid() {
            return (None, focus);
        }
        let mut top_left = POINT { x: info.rcCaret.left, y: info.rcCaret.top };
        let mut bottom_right = POINT { x: info.rcCaret.right, y: info.rcCaret.bottom };
        if !ClientToScreen(info.hwndCaret, &mut top_left).as_bool()
            || !ClientToScreen(info.hwndCaret, &mut bottom_right).as_bool()
        {
            return (None, focus);
        }
        let rect = Rect {
            left: top_left.x,
            top: top_left.y,
            right: bottom_right.x.max(top_left.x + 1),
            bottom: bottom_right.y,
        };
        (Some(rect), focus)
    }
}

fn accessible_caret(window: HWND) -> Option<Rect> {
    unsafe {
        let mut raw: *mut c_void = std::ptr::null_mut();
        AccessibleObjectFromWindow(window, OBJID_CARET.0 as u32, &IAccessible::IID, &mut raw).ok()?;
        if raw.is_null() {
            return None;
        }
        let accessible = IAccessible::from_raw(raw);
        let mut child_self = VARIANT::default();
        {
            let inner = &mut *child_self.Anonymous.Anonymous;
            inner.vt = VT_I4;
            inner.Anonymous.lVal = 0;
        }
        let (mut left, mut top, mut width, mut height) = (0i32, 0i32, 0i32, 0i32);
        accessible
            .accLocation(&mut left, &mut top, &mut width, &mut height, &child_self)
            .ok()?;
        if height <= 0 {
            return None;
        }
        Some(Rect { left, top, right: left + width.max(1), bottom: top + height })
    }
}

fn lookup_caret() -> Option<Rect> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_invalid() {
        return None;
    }
    let host = window_rect(foreground);
    let (from_thread, focus) = gui_thread_caret(foreground);
    if let Some(rect) = from_thread.filter(|r| plausible(*r, host)) {
        return Some(rect);
    }
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let found = accessible_caret(focus).filter(|r| plausible(*r, host));
    if initialized {
        unsafe { CoUninitialize() };
    }
    found
}

fn cursor_anchor() -> Anchor {
    let mut point = POINT::default();
    let _ = unsafe { GetCursorPos(&mut point) };
    let probe = Rect { left: point.x, top: point.y, right: point.x + 1, bottom: point.y + 1 };
    let scale = screen_for(probe).scale;
    Anchor {
        rect: Rect { bottom: point.y + to_physical(CURSOR_HEIGHT_LOGICAL, scale), ..probe },
        from_caret: false,
    }
}

pub fn caret_anchor() -> Anchor {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(lookup_caret());
    });
    match rx.recv_timeout(Duration::from_millis(CARET_LOOKUP_TIMEOUT_MS)) {
        Ok(Some(rect)) => Anchor { rect, from_caret: true },
        _ => cursor_anchor(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect { left: 0, top: 0, right: 1920, bottom: 1040 };

    fn caret_at(x: i32, y: i32) -> Rect {
        Rect { left: x, top: y, right: x + 2, bottom: y + 20 }
    }

    #[test]
    fn the_capsule_sits_just_below_the_caret_with_its_shadow_inset_outside() {
        let p = place(caret_at(400, 300), WORK, (200, 36), 14, 8, None);
        assert!(!p.above);
        assert_eq!((p.x + 14, p.y + 14), (400, 328), "content starts at the caret, a gap below it");
        assert_eq!((p.width, p.height), (228, 64));
    }

    #[test]
    fn near_the_bottom_edge_it_flips_above_the_caret() {
        let p = place(caret_at(400, 1010), WORK, (200, 36), 14, 8, None);
        assert!(p.above);
        assert_eq!(p.y + 14 + 36, 1010 - 8, "content ends a gap above the caret line");
    }

    #[test]
    fn near_the_right_edge_it_slides_left_to_stay_on_screen() {
        let p = place(caret_at(1900, 300), WORK, (300, 36), 14, 8, None);
        assert_eq!(p.x + 14 + 300, 1920, "content's right edge meets the work area's right edge");
    }

    #[test]
    fn on_a_monitor_left_of_the_primary_negative_coordinates_are_kept() {
        let left_monitor = Rect { left: -2560, top: 0, right: 0, bottom: 1400 };
        let p = place(caret_at(-1200, 500), left_monitor, (200, 36), 14, 8, None);
        assert_eq!(p.x + 14, -1200);
        assert!(p.x + p.width - 14 <= 0, "must not spill onto the primary monitor");
    }

    #[test]
    fn a_taskbar_on_top_pushes_the_work_area_and_the_capsule_respects_it() {
        let work = Rect { left: 0, top: 48, right: 1920, bottom: 1080 };
        let p = place(caret_at(200, 40), work, (200, 36), 14, 8, None);
        assert!(p.y + 14 >= 48, "content must not hide under a top taskbar");
    }

    #[test]
    fn a_sticky_side_keeps_the_palette_from_jumping_while_it_grows() {
        let first = place(caret_at(400, 800), WORK, (400, 300), 16, 8, None);
        assert!(first.above, "300 px does not fit below a caret at 800 in a 1040 work area");
        let grown = place(caret_at(400, 800), WORK, (400, 200), 16, 8, Some(true));
        assert!(grown.above, "a shorter list must stay on the side it opened on");
        let unsticky = place(caret_at(400, 800), WORK, (400, 200), 16, 8, None);
        assert!(!unsticky.above, "without the preference it would have jumped below");
    }

    #[test]
    fn content_taller_than_the_screen_is_pinned_to_the_top_instead_of_going_off_screen() {
        let small = Rect { left: 0, top: 0, right: 800, bottom: 300 };
        let p = place(caret_at(100, 150), small, (300, 400), 10, 8, None);
        assert_eq!(p.y + 10, 0);
    }

    #[test]
    fn physical_sizes_scale_with_the_monitor_dpi() {
        assert_eq!(to_physical(36.0, 1.0), 36);
        assert_eq!(to_physical(36.0, 1.5), 54);
        assert_eq!(to_physical(36.0, 1.25), 45);
    }

    #[test]
    fn the_overlay_style_never_activates_and_stays_out_of_the_taskbar_and_alt_tab() {
        let style = overlay_ex_style(WS_EX_APPWINDOW.0);
        assert_ne!(style & WS_EX_NOACTIVATE.0, 0);
        assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
        assert_ne!(style & WS_EX_TOPMOST.0, 0);
        assert_eq!(style & WS_EX_APPWINDOW.0, 0);
        assert_eq!(overlay_ex_style(style), style, "applying it twice changes nothing");
    }

    #[test]
    fn taskbar_and_desktop_windows_are_never_taken_as_the_paste_target() {
        for class in ["Shell_TrayWnd", "Shell_SecondaryTrayWnd", "NotifyIconOverflowWindow", "Progman", "WorkerW"] {
            assert!(is_shell_class(class), "{class}");
        }
        for class in ["Notepad", "Chrome_WidgetWin_1", "OpusApp"] {
            assert!(!is_shell_class(class), "{class}");
        }
    }

    #[test]
    fn a_caret_outside_its_window_is_not_trusted() {
        let host = Some(Rect { left: 100, top: 100, right: 900, bottom: 700 });
        assert!(plausible(caret_at(300, 300), host));
        assert!(!plausible(caret_at(0, 0), host), "a caret at the origin is a stale default");
        assert!(!plausible(Rect { left: 300, top: 300, right: 302, bottom: 300 }, host), "zero height");
    }

    mod win32 {
        use super::*;
        use windows::core::{w, PCWSTR};
        use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, IsWindowVisible,
            PeekMessageW, RegisterClassW, ShowWindow, TranslateMessage, MSG, PM_REMOVE, SW_SHOW,
            WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_POPUP,
        };

        unsafe extern "system" fn proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
            DefWindowProcW(h, m, w, l)
        }

        fn pump() {
            let mut msg = MSG::default();
            unsafe {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        fn create(class: PCWSTR, ex: u32, style: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE) -> HWND {
            unsafe {
                let instance = GetModuleHandleW(None).unwrap();
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(proc),
                    hInstance: instance.into(),
                    lpszClassName: class,
                    ..Default::default()
                };
                RegisterClassW(&wc);
                CreateWindowExW(
                    WINDOW_EX_STYLE(ex),
                    class,
                    w!("probe"),
                    style,
                    100,
                    100,
                    300,
                    80,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
                .unwrap()
            }
        }

        #[test]
        fn showing_and_hiding_the_overlay_never_changes_the_active_or_foreground_window() {
            let control = create(w!("cai-test-control"), 0, WS_OVERLAPPEDWINDOW);
            unsafe {
                let _ = ShowWindow(control, SW_SHOW);
            }
            pump();
            let active_before = unsafe { GetActiveWindow() };
            let foreground_before = foreground();

            let overlay = create(w!("cai-test-overlay"), overlay_ex_style(0), WS_POPUP);
            let raw = overlay.0 as isize;
            show_without_activating(raw, Placement { x: 200, y: 200, width: 240, height: 60, above: false });
            pump();

            assert!(unsafe { IsWindowVisible(overlay) }.as_bool(), "the overlay must actually be shown");
            assert_ne!(unsafe { GetActiveWindow() }, overlay, "the overlay must never become active");
            assert_eq!(unsafe { GetActiveWindow() }, active_before, "the active window must not move");
            assert_ne!(foreground(), raw, "the overlay must never become the foreground window");
            assert_eq!(foreground(), foreground_before);

            show_without_activating(raw, Placement { x: 260, y: 240, width: 300, height: 60, above: false });
            pump();
            assert_eq!(unsafe { GetActiveWindow() }, active_before, "moving it must not activate it");
            let mut rect = RECT::default();
            unsafe { GetWindowRect(overlay, &mut rect) }.unwrap();
            assert_eq!((rect.left, rect.top, rect.right - rect.left), (260, 240, 300));

            hide_without_activating(raw);
            pump();
            assert!(!unsafe { IsWindowVisible(overlay) }.as_bool());
            assert_eq!(unsafe { GetActiveWindow() }, active_before, "hiding it must not hand activation around");
            assert_eq!(foreground(), foreground_before);

            unsafe {
                let _ = DestroyWindow(overlay);
                let _ = DestroyWindow(control);
            }
        }

        #[test]
        fn ensure_overlay_style_repairs_a_style_that_lost_its_no_activate_bit() {
            let window = create(w!("cai-test-restyle"), WS_EX_APPWINDOW.0, WS_POPUP);
            let raw = window.0 as isize;
            ensure_overlay_style(raw);
            let style = unsafe { GetWindowLongPtrW(window, GWL_EXSTYLE) } as u32;
            assert_ne!(style & WS_EX_NOACTIVATE.0, 0);
            assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
            assert_eq!(style & WS_EX_APPWINDOW.0, 0);
            unsafe {
                let _ = DestroyWindow(window);
            }
        }

        #[test]
        fn our_own_windows_are_never_taken_as_the_paste_target() {
            let window = create(w!("cai-test-own"), 0, WS_POPUP);
            assert!(is_own_or_shell(window.0 as isize));
            unsafe {
                let _ = DestroyWindow(window);
            }
        }

        #[test]
        fn activating_a_window_that_no_longer_exists_fails_instead_of_hanging() {
            let window = create(w!("cai-test-gone"), 0, WS_POPUP);
            let raw = window.0 as isize;
            unsafe {
                let _ = DestroyWindow(window);
            }
            let started = Instant::now();
            assert!(!activate(raw, 300));
            assert!(started.elapsed() < Duration::from_millis(100), "a dead window is rejected up front");
            assert!(!activate(0, 300));
        }
    }
}
