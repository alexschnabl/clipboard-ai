use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};
use std::thread::JoinHandle;
use std::time::Duration;

use tauri::image::Image;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

pub const CYCLE_STATE_EVENT: &str = "cycle://state";

fn emit_running(app: &AppHandle, running: bool) {
    let _ = app.emit(CYCLE_STATE_EVENT, running);
}

pub const INDICATOR_WINDOW: &str = "indicator";

const INDICATOR_MARGIN_LOGICAL: f64 = 16.0;

fn foreground_monitor_bottom_right() -> Option<(i32, i32)> {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) };
    if monitor.is_invalid() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        Some((info.rcWork.right, info.rcWork.bottom))
    } else {
        None
    }
}

fn primary_work_area_bottom_right() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut rect = RECT::default();
    unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok()
    .map(|_| (rect.right, rect.bottom))
}

fn show_indicator(app: &AppHandle) {
    let Some(window) = app.get_webview_window(INDICATOR_WINDOW) else { return };
    let corner = foreground_monitor_bottom_right()
        .or_else(primary_work_area_bottom_right)
        .or_else(|| {
            window.primary_monitor().ok().flatten().map(|m| {
                let position = m.position();
                let size = m.size();
                (position.x + size.width as i32, position.y + size.height as i32)
            })
        });
    if let (Some((right, bottom)), Ok(size)) = (corner, window.outer_size()) {
        let scale = window.scale_factor().unwrap_or(1.0);
        let margin = (INDICATOR_MARGIN_LOGICAL * scale).round() as i32;
        let x = right - size.width as i32 - margin;
        let y = bottom - size.height as i32 - margin;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
    let _ = window.show();
}

fn hide_indicator(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(INDICATOR_WINDOW) {
        let _ = window.hide();
    }
}

pub const TRAY_ID: &str = "clipboard-ai-tray";

const SHOW_DELAY_MS: u64 = 250;

const FRAME_INTERVAL_MS: u64 = 90;

const POLL_MS: u64 = 30;

const MIN_ALPHA_SCALE: f64 = 0.4;

const FRAME_COUNT: usize = 14;

const TRAY_ICON_PNG: &[u8] = include_bytes!("../icons/tray-32.png");

pub fn tray_icon() -> Option<Image<'static>> {
    Image::from_bytes(TRAY_ICON_PNG).ok()
}

struct Frames {
    idle: Image<'static>,
    busy: Vec<Image<'static>>,
}

static FRAMES: OnceLock<Option<Frames>> = OnceLock::new();

fn scale_alpha(rgba: &[u8], scale: f64) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let mut i = 3;
    while i < out.len() {
        out[i] = (out[i] as f64 * scale).round().clamp(0.0, 255.0) as u8;
        i += 4;
    }
    out
}

fn breath_scale(i: usize, count: usize) -> f64 {
    let phase = (i as f64 / count as f64) * std::f64::consts::TAU;
    let unit = 0.5 * (1.0 + phase.cos());
    MIN_ALPHA_SCALE + (1.0 - MIN_ALPHA_SCALE) * unit
}

fn build_frames() -> Option<Frames> {
    let icon = tray_icon()?;
    let (rgba, w, h) = (icon.rgba(), icon.width(), icon.height());
    let idle = Image::new_owned(rgba.to_vec(), w, h);
    let busy = (0..FRAME_COUNT)
        .map(|i| Image::new_owned(scale_alpha(rgba, breath_scale(i, FRAME_COUNT)), w, h))
        .collect();
    Some(Frames { idle, busy })
}

fn set_tray_icon(app: &AppHandle, frame: &Image<'_>) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let borrowed = Image::new(frame.rgba(), frame.width(), frame.height());
        let _ = tray.set_icon(Some(borrowed));
    }
}

fn sleep_cancellable(cancel: &AtomicBool, total_ms: u64) -> bool {
    let mut slept = 0;
    while slept < total_ms {
        if cancel.load(Ordering::SeqCst) {
            return true;
        }
        let step = POLL_MS.min(total_ms - slept);
        std::thread::sleep(Duration::from_millis(step));
        slept += step;
    }
    cancel.load(Ordering::SeqCst)
}

fn run_animation(app: AppHandle, cancel: std::sync::Arc<AtomicBool>) {
    if sleep_cancellable(&cancel, SHOW_DELAY_MS) {
        return;
    }
    let Some(Some(frames)) = FRAMES.get() else { return };

    emit_running(&app, true);
    show_indicator(&app);

    let mut i = 0;
    loop {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        set_tray_icon(&app, &frames.busy[i]);
        i = (i + 1) % frames.busy.len();
        if sleep_cancellable(&cancel, FRAME_INTERVAL_MS) {
            break;
        }
    }
    set_tray_icon(&app, &frames.idle);
    hide_indicator(&app);
    emit_running(&app, false);
}

pub struct TrayAnimation {
    app: AppHandle,
    cancel: std::sync::Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TrayAnimation {
    pub fn begin(app: AppHandle) -> Self {
        let has_frames = FRAMES.get_or_init(build_frames).is_some();
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let handle = if has_frames {
            let app = app.clone();
            let cancel = cancel.clone();
            Some(std::thread::spawn(move || run_animation(app, cancel)))
        } else {
            None
        };
        Self { app, cancel, handle }
    }

    pub fn finish(self) {}
}

impl Drop for TrayAnimation {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            if handle.join().is_err() {
                if let Some(Some(frames)) = FRAMES.get() {
                    set_tray_icon(&self.app, &frames.idle);
                }
                hide_indicator(&self.app);
                emit_running(&self.app, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breath_scale_is_full_at_the_start_and_dimmest_at_the_midpoint() {
        assert!((breath_scale(0, FRAME_COUNT) - 1.0).abs() < 1e-9, "frame 0 is full opacity");
        let mid = breath_scale(FRAME_COUNT / 2, FRAME_COUNT);
        assert!(
            (mid - MIN_ALPHA_SCALE).abs() < 1e-9,
            "the midpoint frame is the dimmest, got {mid}"
        );
    }

    #[test]
    fn breath_scale_never_leaves_the_visible_range() {
        for i in 0..FRAME_COUNT {
            let s = breath_scale(i, FRAME_COUNT);
            assert!(
                (MIN_ALPHA_SCALE..=1.0).contains(&s),
                "frame {i} scale {s} escaped [{MIN_ALPHA_SCALE}, 1.0] -- the glyph would vanish or over-brighten"
            );
        }
    }

    #[test]
    fn scale_alpha_touches_only_the_alpha_byte_and_preserves_color() {
        let px = [10u8, 120, 90, 200];
        let out = scale_alpha(&px, 0.5);
        assert_eq!(&out[0..3], &[10, 120, 90], "color channels must be untouched");
        assert_eq!(out[3], 100, "alpha must scale");
    }

    #[test]
    fn scale_alpha_at_full_scale_is_the_identity() {
        let rgba = [1u8, 2, 3, 255, 4, 5, 6, 128];
        assert_eq!(scale_alpha(&rgba, 1.0), rgba.to_vec());
    }

    #[test]
    fn the_embedded_tray_icon_decodes_to_a_non_empty_square() {
        let icon = tray_icon().expect("the embedded tray PNG must decode");
        assert_eq!(icon.width(), icon.height(), "the tray icon should be square");
        assert!(icon.width() >= 16, "a tray icon smaller than 16px would not render usefully");
        assert_eq!(
            icon.rgba().len() as u32,
            icon.width() * icon.height() * 4,
            "decoded RGBA length must match the dimensions"
        );
    }

    #[test]
    fn a_cancelled_sleep_returns_true_without_waiting_the_whole_duration() {
        let cancel = AtomicBool::new(true);
        assert!(sleep_cancellable(&cancel, 10_000), "an already-set cancel must short-circuit");
    }

    #[test]
    fn an_uncancelled_sleep_runs_to_completion() {
        let cancel = AtomicBool::new(false);
        assert!(!sleep_cancellable(&cancel, 20), "no cancel -> false (ran the full duration)");
    }
}
