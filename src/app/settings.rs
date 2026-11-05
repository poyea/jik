// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use anyhow::{Context, Result};
use glasskit::Ui;

use super::APP_NAME;
use crate::config_store::{Config, load_config, save_config};
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
///
/// The window shows the files as they are on disk now, so a save does not undo changes that
/// the user made to the files by hand while jik runs. If a file cannot be read, the window
/// shows the settings that jik uses (`config` and `current`), and a save replaces the files.
pub fn open(
    ui: &Ui,
    files: &Files,
    config: &Config,
    autostart: bool,
    current: &[Question],
) -> Result<Opened> {
    let (config, current) = match read_files(files) {
        Ok(on_disk) => on_disk,
        Err(err) => {
            glasskit::show_error(
                APP_NAME,
                &format!(
                    "{err:#}

The Settings window shows the settings that jik uses now.                      Save replaces the files with them."
                ),
            );
            (config.clone(), current.to_vec())
        }
    };
    match settings::edit(ui, &config, autostart, &current)? {
        SettingsOutcome::Cancelled => Ok(Opened::Unchanged),
        SettingsOutcome::Quit => Ok(Opened::Quit),
        SettingsOutcome::Saved(edited) => {
            save_config(&files.config, &edited.config)?;
            questions::save(&files.questions, &edited.questions)?;
            Ok(Opened::Saved(edited))
        }
    }
}

fn read_files(files: &Files) -> Result<(Config, Vec<Question>)> {
    Ok((
        load_config(&files.config)?,
        questions::load_or_create(&files.questions)?,
    ))
}

pub fn set_autostart(enabled: bool) -> Result<()> {
    glasskit::autostart::set_enabled(APP_NAME, enabled)
        .context("could not change Start with Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_store::{Position, Preset};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jik-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn settings_read_the_files_as_they_are_now() {
        let dir = temp_dir("settings-read");
        let files = Files {
            config: dir.join("config.toml"),
            questions: dir.join("questions.toml"),
        };
        std::fs::write(
            &files.config,
            "[popup]
position = \"top-left\"
",
        )
        .unwrap();
        std::fs::write(
            &files.questions,
            "[[question]]
id = \"q\"
text = \"Edited by hand?\"
choices = [\"yes\", \"no\"]
answer = \"yes\"
",
        )
        .unwrap();
        let (config, questions) = read_files(&files).unwrap();
        assert_eq!(config.popup.position, Position::Named(Preset::TopLeft));
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].text, "Edited by hand?");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_broken_file_is_an_error() {
        let dir = temp_dir("settings-broken");
        let files = Files {
            config: dir.join("config.toml"),
            questions: dir.join("questions.toml"),
        };
        std::fs::write(
            &files.config,
            "[popup]
position = 42
",
        )
        .unwrap();
        assert!(read_files(&files).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
