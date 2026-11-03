// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, TRUE, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::{
    CancelWaitableTimer, CreateWaitableTimerExW, INFINITE, SetWaitableTimer, TIMER_ALL_ACCESS,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetSystemMetrics, HICON, HMENU,
    ICONINFO, MENU_ITEM_FLAGS, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MSG,
    MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, PostMessageW,
    PostQuitMessage, QS_ALLINPUT, RegisterClassExW, RegisterWindowMessageW, SM_CXSMICON,
    SetForegroundWindow, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTALIGN,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP, WM_LBUTTONUP, WM_NULL, WM_QUIT,
    WM_RBUTTONUP, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{BOOL, HSTRING, PCSTR, PCWSTR, w};

const CLASS_NAME: PCWSTR = w!("WinkitTray");
/// The numbers of the uxtheme.dll exports for dark menus. They have no names.
const SET_PREFERRED_APP_MODE: u16 = 135;
const ALLOW_DARK_MODE_FOR_WINDOW: u16 = 133;
const FLUSH_MENU_THEMES: u16 = 136;
/// The app mode that follows the Windows light or dark setting.
const APP_MODE_ALLOW_DARK: i32 = 1;
const WM_TRAY: u32 = WM_APP + 1;
const ICON_ID: u32 = 1;
const FIRST_ID: usize = 1;
/// The number of 100 ns intervals from 1601-01-01 (FILETIME start) to 1970-01-01 (Unix start).
const UNIX_EPOCH_AS_FILETIME: i64 = 116_444_736_000_000_000;

/// A positive due time is absolute for `SetWaitableTimer`, and a negative one is relative.
/// Thus the value must stay positive: it saturates and never wraps.
fn absolute_due_time(at: SystemTime) -> i64 {
    let since_unix = at.duration_since(UNIX_EPOCH).unwrap_or_default();
    i64::try_from(since_unix.as_nanos().div_ceil(100))
        .unwrap_or(i64::MAX)
        .saturating_add(UNIX_EPOCH_AS_FILETIME)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    Command {
        id: u32,
        label: String,
        enabled: bool,
        checked: bool,
    },
    Label(String),
    Separator,
    Submenu {
        label: String,
        items: Vec<MenuItem>,
    },
    Quit(String),
}

impl MenuItem {
    pub fn command(id: u32, label: impl Into<String>) -> Self {
        MenuItem::Command {
            id,
            label: label.into(),
            enabled: true,
            checked: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    Deadline,
    Command(u32),
    Quit,
}

pub struct IconImage {
    pub size: u32,
    pub pixels: Vec<u32>,
}

pub fn icon_size() -> u32 {
    unsafe { GetSystemMetrics(SM_CXSMICON).max(16) as u32 }
}

thread_local! {
    static COMMANDS: RefCell<VecDeque<u32>> = const { RefCell::new(VecDeque::new()) };
    static MENU: RefCell<Vec<MenuItem>> = const { RefCell::new(Vec::new()) };
    static ICON_DATA: RefCell<Option<NOTIFYICONDATAW>> = const { RefCell::new(None) };
    static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
}

/// An absolute timer fires at the correct clock time after sleep, hibernation, or a clock change.
/// Thus the wait does not have to wake up to check the clock.
struct DeadlineTimer(HANDLE);

impl DeadlineTimer {
    fn new() -> Result<Self> {
        let handle = unsafe {
            CreateWaitableTimerExW(None, PCWSTR::null(), 0, TIMER_ALL_ACCESS.0)
                .context("CreateWaitableTimerExW")?
        };
        Ok(Self(handle))
    }

    fn set(&self, at: SystemTime) -> Result<()> {
        let due = absolute_due_time(at);
        unsafe { SetWaitableTimer(self.0, &due, 0, None, None, false).context("SetWaitableTimer") }
    }

    fn cancel(&self) {
        unsafe {
            let _ = CancelWaitableTimer(self.0);
        }
    }
}

impl Drop for DeadlineTimer {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub struct Tray {
    hwnd: HWND,
    icon: HICON,
    timer: DeadlineTimer,
}

impl Tray {
    pub fn new(tooltip: &str, icon: &IconImage) -> Result<Self> {
        let timer = DeadlineTimer::new()?;
        unsafe {
            let hinstance: HINSTANCE = GetModuleHandleW(None).context("GetModuleHandleW")?.into();
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance,
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            if RegisterClassExW(&wc) == 0 {
                return Err(windows::core::Error::from_thread()).context("RegisterClassExW(tray)");
            }
            // Use a hidden top-level window, not a message-only window.
            // Only a top-level window gets the TaskbarCreated message when Explorer restarts.
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                CLASS_NAME,
                CLASS_NAME,
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(hinstance),
                None,
            )
            .context("CreateWindowExW(tray)")?;
            TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
            allow_dark_menus(hwnd);

            let icon = match create_icon(icon) {
                Ok(icon) => icon,
                Err(err) => {
                    let _ = DestroyWindow(hwnd);
                    return Err(err);
                }
            };
            let mut data = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: ICON_ID,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                hIcon: icon,
                ..Default::default()
            };
            set_tip(&mut data, tooltip);
            ICON_DATA.set(Some(data));
            if !Shell_NotifyIconW(NIM_ADD, &data).as_bool() {
                ICON_DATA.set(None);
                let _ = DestroyIcon(icon);
                let _ = DestroyWindow(hwnd);
                anyhow::bail!("could not add the tray icon");
            }
            Ok(Self { hwnd, icon, timer })
        }
    }

    pub fn set_menu(&self, items: Vec<MenuItem>) {
        MENU.set(items);
    }

    pub fn set_tooltip(&self, tooltip: &str) {
        ICON_DATA.with_borrow_mut(|data| {
            if let Some(data) = data {
                set_tip(data, tooltip);
                unsafe {
                    let _ = Shell_NotifyIconW(NIM_MODIFY, data);
                }
            }
        });
    }

    pub fn wait(&self, until: Option<SystemTime>) -> Result<Wake> {
        if until.is_none() {
            self.timer.cancel();
        }
        loop {
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        return Ok(Wake::Quit);
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            if let Some(id) = COMMANDS.with_borrow_mut(|q| q.pop_front()) {
                return Ok(Wake::Command(id));
            }
            if let Some(t) = until {
                if SystemTime::now() >= t {
                    return Ok(Wake::Deadline);
                }
                self.timer.set(t)?;
            }
            unsafe {
                MsgWaitForMultipleObjectsEx(
                    Some(&[self.timer.0]),
                    INFINITE,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
            }
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            ICON_DATA.with_borrow_mut(|data| {
                if let Some(data) = data.take() {
                    let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                }
            });
            let _ = DestroyIcon(self.icon);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn set_tip(data: &mut NOTIFYICONDATAW, tip: &str) {
    let max = data.szTip.len() - 1;
    let wide: Vec<u16> = tip.encode_utf16().take(max).collect();
    data.szTip = [0; 128];
    data.szTip[..wide.len()].copy_from_slice(&wide);
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_LBUTTONUP || event == WM_RBUTTONUP {
                show_menu(hwnd);
            }
            LRESULT(0)
        }
        m if m != 0 && m == TASKBAR_CREATED.get() => {
            ICON_DATA.with_borrow(|data| {
                if let Some(data) = data {
                    unsafe {
                        let _ = Shell_NotifyIconW(NIM_ADD, data);
                    }
                }
            });
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[derive(Clone, Copy)]
enum Action {
    Command(u32),
    Quit,
}

fn register(actions: &mut Vec<Action>, action: Action) -> usize {
    actions.push(action);
    FIRST_ID + actions.len() - 1
}

fn append_items(menu: HMENU, items: &[MenuItem], actions: &mut Vec<Action>) {
    let flag = |on: bool, f: MENU_ITEM_FLAGS| if on { f } else { MENU_ITEM_FLAGS(0) };
    unsafe {
        for item in items {
            match item {
                MenuItem::Command {
                    id,
                    label,
                    enabled,
                    checked,
                } => {
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING | flag(!enabled, MF_GRAYED) | flag(*checked, MF_CHECKED),
                        register(actions, Action::Command(*id)),
                        &HSTRING::from(label),
                    );
                }
                MenuItem::Label(label) => {
                    let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, &HSTRING::from(label));
                }
                MenuItem::Separator => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                }
                MenuItem::Submenu { label, items } => {
                    if let Ok(sub) = CreatePopupMenu() {
                        append_items(sub, items, actions);
                        let _ = AppendMenuW(menu, MF_POPUP, sub.0 as usize, &HSTRING::from(label));
                    }
                }
                MenuItem::Quit(label) => {
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING,
                        register(actions, Action::Quit),
                        &HSTRING::from(label),
                    );
                }
            }
        }
    }
}

fn show_menu(hwnd: HWND) {
    let items = MENU.with_borrow(|m| m.clone());
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let mut actions = Vec::new();
        append_items(menu, &items, &mut actions);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // This is necessary. Without it, the menu does not close when the user clicks outside it.
        let _ = SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RIGHTALIGN,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        );
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);

        let index = (chosen.0 as usize).wrapping_sub(FIRST_ID);
        match actions.get(index) {
            Some(Action::Command(id)) => COMMANDS.with_borrow_mut(|q| q.push_back(*id)),
            Some(Action::Quit) => PostQuitMessage(0),
            None => {}
        }
    }
}

fn create_icon(image: &IconImage) -> Result<HICON> {
    let size = image.size as i32;
    anyhow::ensure!(
        image.pixels.len() == (image.size * image.size) as usize,
        "icon pixel count does not match its size"
    );
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let color = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)
            .context("CreateDIBSection")?;
        std::slice::from_raw_parts_mut(bits as *mut u32, image.pixels.len())
            .copy_from_slice(&image.pixels);

        let stride = ((size + 15) / 16 * 2) as usize;
        let mask_bits = vec![0u8; stride * size as usize];
        let mask = CreateBitmap(size, size, 1, 1, Some(mask_bits.as_ptr().cast()));
        let icon = CreateIconIndirect(&ICONINFO {
            fIcon: TRUE,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        });
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon.context("CreateIconIndirect")
    }
}

/// Make the menus follow the Windows dark mode, as the menus of File Explorer do.
/// Windows has no public API for this. uxtheme.dll exports the functions only by number, since
/// Windows 10 version 1903. If a function is missing, the menus stay light.
fn allow_dark_menus(hwnd: HWND) {
    unsafe {
        let Ok(uxtheme) = LoadLibraryW(w!("uxtheme.dll")) else {
            return;
        };
        let export = |number: u16| GetProcAddress(uxtheme, PCSTR(number as usize as *const u8));
        if let Some(f) = export(SET_PREFERRED_APP_MODE) {
            let set: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(f);
            set(APP_MODE_ALLOW_DARK);
        }
        if let Some(f) = export(ALLOW_DARK_MODE_FOR_WINDOW) {
            let allow: unsafe extern "system" fn(HWND, BOOL) -> BOOL = std::mem::transmute(f);
            let _ = allow(hwnd, TRUE);
        }
        if let Some(f) = export(FLUSH_MENU_THEMES) {
            let flush: unsafe extern "system" fn() = std::mem::transmute(f);
            flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn due_time_counts_from_1601() {
        assert_eq!(absolute_due_time(UNIX_EPOCH), UNIX_EPOCH_AS_FILETIME);
        let one_second = UNIX_EPOCH + Duration::from_secs(1);
        assert_eq!(
            absolute_due_time(one_second),
            UNIX_EPOCH_AS_FILETIME + 10_000_000
        );
    }

    #[test]
    fn due_time_stays_positive_far_in_the_future() {
        const YEAR: u64 = 365 * 24 * 60 * 60;
        let far = UNIX_EPOCH
            .checked_add(Duration::from_secs(20_000 * YEAR))
            .expect("SystemTime holds 20000 years");
        assert!(absolute_due_time(far) > UNIX_EPOCH_AS_FILETIME);
    }

    #[test]
    fn due_time_before_1970_is_1970() {
        let before = UNIX_EPOCH - Duration::from_secs(60);
        assert_eq!(absolute_due_time(before), UNIX_EPOCH_AS_FILETIME);
    }
}
