// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

//! Use all UI types on one thread. That thread must run the message loops
//! in [`popup::ChoicePopup::ask`] and [`tray::Tray::wait`].

pub mod autostart;
pub mod popup;
pub mod process;
pub mod tray;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::HSTRING;

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
            .context("CreateMutexW")?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return Ok(None);
        }
        Ok(Some(InstanceGuard(handle)))
    }
}
