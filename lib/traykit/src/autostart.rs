// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use anyhow::{Context, Result};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{HSTRING, PCWSTR, w};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

fn command() -> Result<String> {
    let exe = std::env::current_exe().context("locating the executable")?;
    Ok(format!("\"{}\"", exe.display()))
}

pub fn is_enabled(name: &str) -> bool {
    let Ok(expected) = command() else {
        return false;
    };
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * size_of::<u16>()) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            &HSTRING::from(name),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return false;
    }
    let len = (size as usize / size_of::<u16>()).saturating_sub(1);
    String::from_utf16_lossy(&buf[..len]).eq_ignore_ascii_case(&expected)
}

pub fn set_enabled(name: &str, enabled: bool) -> Result<()> {
    let name = HSTRING::from(name);
    unsafe {
        if enabled {
            let wide: Vec<u16> = command()?.encode_utf16().chain([0]).collect();
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                &name,
                REG_SZ.0,
                Some(wide.as_ptr().cast()),
                (wide.len() * size_of::<u16>()) as u32,
            )
            .ok()
            .context("writing the Run registry entry")
        } else {
            let status = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN_KEY, &name);
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            status.ok().context("removing the Run registry entry")
        }
    }
}
