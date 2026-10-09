//! Telas e janelas que dá para capturar (Windows Graphics Capture).

use serde::Serialize;
use windows::core::{BOOL, PCWSTR};
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, DEVMODEW, ENUM_CURRENT_SETTINGS, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, GA_ROOTOWNER, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
};

use crate::audio::apps::ProcessTable;
use crate::audio::display_name;

const MONITORINFOF_PRIMARY: u32 = 1;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScreenSource {
    /// `screen:\\.\DISPLAY2`
    pub id: String,
    /// Número que o Windows dá à tela (o do `\\.\DISPLAYn`).
    pub number: u32,
    /// Nome do monitor (ex.: "27G2G4"); vazio se o Windows não souber.
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub primary: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WindowSource {
    /// `window:<hwnd>`
    pub id: String,
    pub title: String,
    /// Nome do programa (ex.: "Discord").
    pub app: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSources {
    pub screens: Vec<ScreenSource>,
    pub windows: Vec<WindowSource>,
}

fn wide_to_string(wide: &[u16]) -> String {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}

/// `\\.\DISPLAY2` → nome do monitor ("27G2G4"), pelo DisplayConfig.
fn monitor_names() -> Vec<(String, String)> {
    let mut names = Vec::new();
    unsafe {
        let (mut path_count, mut mode_count) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count).is_err() {
            return names;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut path_count, paths.as_mut_ptr(), &mut mode_count, modes.as_mut_ptr(), None).is_err() {
            return names;
        }
        for path in &paths[..path_count as usize] {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            source.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            source.header.adapterId = path.sourceInfo.adapterId;
            source.header.id = path.sourceInfo.id;
            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            target.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            target.header.adapterId = path.targetInfo.adapterId;
            target.header.id = path.targetInfo.id;
            if DisplayConfigGetDeviceInfo(&mut source.header) == 0 && DisplayConfigGetDeviceInfo(&mut target.header) == 0 {
                names.push((wide_to_string(&source.viewGdiDeviceName), wide_to_string(&target.monitorFriendlyDeviceName)));
            }
        }
    }
    names
}

unsafe extern "system" fn collect_monitor(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
    unsafe { (*(data.0 as *mut Vec<HMONITOR>)).push(monitor) };
    true.into()
}

pub fn monitor_handles() -> Vec<HMONITOR> {
    let mut monitors: Vec<HMONITOR> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(collect_monitor), LPARAM(&mut monitors as *mut _ as isize));
    }
    monitors
}

/// Nome de dispositivo (`\\.\DISPLAY2`) de um monitor.
pub fn monitor_device(monitor: HMONITOR) -> Option<(String, bool)> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    unsafe { GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) }.as_bool().then(|| {
        (wide_to_string(&info.szDevice), info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0)
    })
}

pub fn list_screens() -> Vec<ScreenSource> {
    let names = monitor_names();
    let mut screens: Vec<ScreenSource> = monitor_handles()
        .into_iter()
        .filter_map(|monitor| {
            let (device, primary) = monitor_device(monitor)?;
            let wide: Vec<u16> = device.encode_utf16().chain([0]).collect();
            let mut mode = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
            unsafe { EnumDisplaySettingsW(PCWSTR(wide.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode) }.ok().ok()?;
            let number = device.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(0);
            let name = names.iter().find(|(gdi, _)| gdi.eq_ignore_ascii_case(&device)).map(|(_, name)| name.clone()).unwrap_or_default();
            Some(ScreenSource {
                id: format!("screen:{device}"),
                number,
                name,
                width: mode.dmPelsWidth,
                height: mode.dmPelsHeight,
                refresh_rate: mode.dmDisplayFrequency,
                primary,
            })
        })
        .collect();
    screens.sort_by_key(|s| s.number);
    screens
}

unsafe extern "system" fn collect_window(hwnd: HWND, data: LPARAM) -> BOOL {
    unsafe { (*(data.0 as *mut Vec<HWND>)).push(hwnd) };
    true.into()
}

/// Janelas que aparecem na barra de tarefas (visíveis, com título, sem ser ferramenta/oculta).
pub fn list_windows() -> Vec<WindowSource> {
    let mut handles: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect_window), LPARAM(&mut handles as *mut _ as isize));
    }
    let processes = ProcessTable::snapshot().ok();
    let own = std::process::id();
    handles
        .into_iter()
        .filter_map(|hwnd| unsafe {
            if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() || GetAncestor(hwnd, GA_ROOTOWNER) != hwnd {
                return None;
            }
            if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
                return None;
            }
            let mut cloaked = 0u32;
            if DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut _, 4).is_ok() && cloaked != 0 {
                return None;
            }
            let length = GetWindowTextLengthW(hwnd);
            if length == 0 {
                return None;
            }
            let mut class = [0u16; 64];
            let class_length = GetClassNameW(hwnd, &mut class) as usize;
            let class = String::from_utf16_lossy(&class[..class_length]);
            if matches!(class.as_str(), "Progman" | "Shell_TrayWnd" | "WorkerW") {
                return None;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == own {
                return None;
            }
            let mut title = vec![0u16; length as usize + 1];
            let copied = GetWindowTextW(hwnd, &mut title) as usize;
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            let exe = processes.as_ref().and_then(|p| p.exe(pid)).unwrap_or_default();
            Some(WindowSource {
                id: format!("window:{}", hwnd.0 as isize),
                title: String::from_utf16_lossy(&title[..copied]),
                app: if exe.is_empty() { String::new() } else { display_name(exe) },
                width: (rect.right - rect.left).max(0) as u32,
                height: (rect.bottom - rect.top).max(0) as u32,
            })
        })
        .collect()
}

pub fn list() -> CaptureSources {
    CaptureSources { screens: list_screens(), windows: list_windows() }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn list_capture_sources() {
        let sources = super::list();
        for screen in &sources.screens {
            println!("{screen:?}");
        }
        for window in &sources.windows {
            println!("{window:?}");
        }
    }
}
