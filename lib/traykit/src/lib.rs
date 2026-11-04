// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

//! Use all UI types on one thread. That thread must run the message loops
//! in [`popup::ChoicePopup::ask`], [`dialog::Dialog::run`], and [`tray::Tray::wait`].

pub mod autostart;
pub mod dialog;
pub mod error;
pub mod placement;
pub mod popup;
pub mod presence;
pub mod process;
mod theme;
pub mod tray;
mod ui;

pub use error::{Error, Result};
pub use ui::Ui;

use crate::error::Call;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HANDLE, POINT,
};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForSystem, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_OK, MessageBoxW, RegisterClassExW, WNDCLASSEXW,
};
use windows::core::HSTRING;

/// Register a window class. A class stays registered until the process ends, so a second
/// registration of the same class is not an error.
pub(crate) fn register_class(class: &WNDCLASSEXW) -> Result<()> {
    unsafe {
        if RegisterClassExW(class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(Error::last("RegisterClassExW"));
        }
    }
    Ok(())
}

/// The system DPI stays at its sign-in value. Read the DPI of the monitor each time a window
/// opens, so a change of the display scale applies to the next window.
pub(crate) fn primary_monitor_dpi() -> u32 {
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let (mut dpi_x, mut dpi_y) = (0u32, 0u32);
        match GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) {
            Ok(()) if dpi_x > 0 => dpi_x,
            _ => GetDpiForSystem(),
        }
    }
}

pub fn show_error(title: &str, message: &str) {
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from(title),
            MB_OK | MB_ICONERROR,
        );
    }
}

pub struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn single_instance(name: &str) -> Result<Option<InstanceGuard>> {
    unsafe {
        let handle = CreateMutexW(None, true, &HSTRING::from(format!("Local\\{name}")))
            .call("CreateMutexW")?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return Ok(None);
        }
        Ok(Some(InstanceGuard(handle)))
    }
}
