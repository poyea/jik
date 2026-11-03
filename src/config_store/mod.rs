// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

mod config;
mod setting;

pub use config::{Config, PopupConfig, Position};
pub use setting::{Settings, save_config, with_line_endings_of};
