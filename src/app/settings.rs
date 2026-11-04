// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use anyhow::{Context, Result};

use super::APP_NAME;
use crate::config_store::{Config, save_config};
use crate::study::questions::{self, Question};
use crate::ui::settings::{self, Edited, SettingsOutcome};

pub struct Files {
    pub config: PathBuf,
    pub questions: PathBuf,
}

pub enum Opened {
    Saved(Edited),
    Unchanged,
    Quit,
}

/// Show the settings window. If the user saves, write `config.toml` and `questions.toml`.
pub fn open(
    files: &Files,
    config: &Config,
    autostart: bool,
    current: &[Question],
) -> Result<Opened> {
    match settings::edit(config, autostart, current)? {
        SettingsOutcome::Cancelled => Ok(Opened::Unchanged),
        SettingsOutcome::Quit => Ok(Opened::Quit),
        SettingsOutcome::Saved(edited) => {
            save_config(&files.config, &edited.config)?;
            questions::save(&files.questions, &edited.questions)?;
            Ok(Opened::Saved(edited))
        }
    }
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    traykit::autostart::set_enabled(APP_NAME, enabled)
        .context("could not change Start with Windows")
}
