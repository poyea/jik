// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

mod run;
mod session;
mod settings;

pub use run::run_tray;
pub use session::Session;
pub use settings::{Files, Opened, open as open_settings, set_autostart};

pub const APP_NAME: &str = "jik";
