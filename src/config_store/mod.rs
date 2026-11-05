// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

mod config;
mod schedule;
mod setting;

pub use config::{Config, PopupConfig, Position, Preset};
pub use setting::{Settings, load_config, save_config};
