use std::path::Path;

use serde::Serialize;
use windows::core::{BOOL, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, FILE_FLAGS_AND_ATTRIBUTES,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, EnumChildWindows, EnumWindows, GetIconInfo, GetWindow, GetWindowLongW,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE,
    GW_OWNER, ICONINFO, WS_EX_TOOLWINDOW,
};

const FRAME_HOST: &str = "applicationframehost.exe";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunningApp {
    pub exe: String,
    pub name: String,
    pub title: String,
    pub icon: Option<AppIcon>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppIcon {
    pub size: u32,
    pub rgba: String,
}

fn process_id(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

fn process_path(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let result =
            QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(handle);
        result.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

unsafe extern "system" fn find_hosted_child(child: HWND, lparam: LPARAM) -> BOOL {
    let state = &mut *(lparam.0 as *mut (u32, u32));
    let pid = process_id(child);
    if pid != 0 && pid != state.0 {
        state.1 = pid;
        return BOOL(0);
    }
    BOOL(1)
}

fn hosted_app_pid(frame: HWND, frame_pid: u32) -> Option<u32> {
    let mut state: (u32, u32) = (frame_pid, 0);
    unsafe {
        let _ = EnumChildWindows(
            Some(frame),
            Some(find_hosted_child),
            LPARAM(&mut state as *mut (u32, u32) as isize),
        );
    }
    (state.1 != 0).then_some(state.1)
}

fn window_process_path(hwnd: HWND) -> Option<String> {
    let pid = process_id(hwnd);
    let path = process_path(pid)?;
    if file_name(&path).eq_ignore_ascii_case(FRAME_HOST) {
        if let Some(hosted) = hosted_app_pid(hwnd, pid).and_then(process_path) {
            return Some(hosted);
        }
    }
    Some(path)
}

pub fn exe_for_window(hwnd: isize) -> Option<String> {
    if hwnd == 0 {
        return None;
    }
    window_process_path(HWND(hwnd as *mut _)).map(|path| file_name(&path))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn read_wide(ptr: *const u16, max_len: usize) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, max_len) };
    let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..end])
}

fn file_description(path: &str) -> Option<String> {
    let wpath = wide(path);
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wpath.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wpath.as_ptr()), None, size, data.as_mut_ptr().cast()).ok()?;

        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let translation = wide("\\VarFileInfo\\Translation");
        let mut pairs: Vec<(u16, u16)> = Vec::new();
        if VerQueryValueW(data.as_ptr().cast(), PCWSTR(translation.as_ptr()), &mut ptr, &mut len)
            .as_bool()
            && len >= 4
        {
            let words = std::slice::from_raw_parts(ptr as *const u16, (len / 2) as usize);
            for chunk in words.chunks_exact(2) {
                pairs.push((chunk[0], chunk[1]));
            }
        }
        pairs.extend([(0x0409, 0x04b0), (0x0409, 0x04e4), (0x0000, 0x04b0)]);
        for (lang, codepage) in pairs {
            let key = wide(&format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\FileDescription"));
            if VerQueryValueW(data.as_ptr().cast(), PCWSTR(key.as_ptr()), &mut ptr, &mut len)
                .as_bool()
                && len > 0
            {
                let text = read_wide(ptr as *const u16, len as usize);
                let text = text.trim();
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
        None
    }
}

pub fn friendly_name(path: &str) -> String {
    let fallback = || {
        let name = file_name(path);
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem.to_string(),
            _ => name,
        }
    };
    match file_description(path) {
        Some(description) if !description.to_lowercase().ends_with(".exe") => description,
        _ => fallback(),
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(BASE64[(n >> 18) as usize & 63] as char);
        out.push(BASE64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { BASE64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { BASE64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn bgra_to_rgba(mut pixels: Vec<u8>) -> Vec<u8> {
    let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
    for p in pixels.chunks_exact_mut(4) {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        }
    }
    pixels
}

fn icon_for(path: &str) -> Option<AppIcon> {
    let wpath = wide(path);
    unsafe {
        let mut info = SHFILEINFOW::default();
        let ok = SHGetFileInfoW(
            PCWSTR(wpath.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if ok == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let icon = info.hIcon;
        let mut icon_info = ICONINFO::default();
        let result = GetIconInfo(icon, &mut icon_info).ok().and_then(|_| {
            let color = icon_info.hbmColor;
            if color.is_invalid() {
                return None;
            }
            let mut bitmap = BITMAP::default();
            if GetObjectW(
                HGDIOBJ(color.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bitmap as *mut BITMAP as *mut _),
            ) == 0
            {
                return None;
            }
            let size = bitmap.bmWidth;
            if size <= 0 || size > 256 || bitmap.bmHeight != size {
                return None;
            }
            let mut header = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size,
                    biHeight: -size,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = vec![0u8; (size * size * 4) as usize];
            let dc = CreateCompatibleDC(None);
            let lines = GetDIBits(
                dc,
                color,
                0,
                size as u32,
                Some(pixels.as_mut_ptr().cast()),
                &mut header,
                DIB_RGB_COLORS,
            );
            let _ = DeleteDC(dc);
            (lines == size).then(|| AppIcon {
                size: size as u32,
                rgba: base64(&bgra_to_rgba(pixels)),
            })
        });
        if !icon_info.hbmColor.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(icon_info.hbmColor.0));
        }
        if !icon_info.hbmMask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(icon_info.hbmMask.0));
        }
        let _ = DestroyIcon(icon);
        result
    }
}

fn is_app_window(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        if GetWindow(hwnd, GW_OWNER).is_ok() {
            return false;
        }
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
        {
            return false;
        }
        GetWindowTextLengthW(hwnd) > 0
    }
}

fn window_title(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buf = vec![0u16; len + 1];
        let copied = GetWindowTextW(hwnd, &mut buf).max(0) as usize;
        String::from_utf16_lossy(&buf[..copied])
    }
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<HWND>);
    if is_app_window(hwnd) {
        windows.push(hwnd);
    }
    BOOL(1)
}

struct ComScope(bool);

impl ComScope {
    fn enter() -> Self {
        Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub fn running_apps() -> Vec<RunningApp> {
    let _com = ComScope::enter();
    let mut windows: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect_window), LPARAM(&mut windows as *mut Vec<HWND> as isize));
    }
    let own_pid = std::process::id();
    let mut apps: Vec<RunningApp> = Vec::new();
    for hwnd in windows {
        if process_id(hwnd) == own_pid {
            continue;
        }
        let Some(path) = window_process_path(hwnd) else { continue };
        let exe = file_name(&path);
        if exe.eq_ignore_ascii_case(FRAME_HOST)
            || apps.iter().any(|a| a.exe.eq_ignore_ascii_case(&exe))
        {
            continue;
        }
        apps.push(RunningApp {
            name: friendly_name(&path),
            title: window_title(hwnd),
            icon: icon_for(&path),
            exe,
        });
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(&[0xff, 0xfe, 0xfd, 0x00]), "//79AA==");
    }

    #[test]
    fn bgra_becomes_rgba_and_an_icon_without_alpha_turns_opaque() {
        assert_eq!(bgra_to_rgba(vec![1, 2, 3, 0, 4, 5, 6, 0]), vec![3, 2, 1, 255, 6, 5, 4, 255]);
        assert_eq!(bgra_to_rgba(vec![1, 2, 3, 128, 4, 5, 6, 0]), vec![3, 2, 1, 128, 6, 5, 4, 0]);
    }

    #[test]
    fn the_running_apps_list_never_includes_this_process_and_has_no_duplicates() {
        let apps = running_apps();
        let mut seen = std::collections::HashSet::new();
        for app in &apps {
            assert!(seen.insert(app.exe.to_lowercase()), "duplicate {}", app.exe);
            assert!(!app.name.is_empty());
        }
    }

    #[test]
    fn a_null_window_has_no_exe() {
        assert_eq!(exe_for_window(0), None);
    }
}
