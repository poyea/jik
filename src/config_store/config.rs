// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub popup: PopupConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PopupConfig {
    pub position: Position,
    /// 0 turns auto-skip off.
    pub auto_skip_seconds: u32,
}

impl Default for PopupConfig {
    fn default() -> Self {
        Self {
            position: Position::default(),
            auto_skip_seconds: 30,
        }
    }
}

impl PopupConfig {
    pub fn auto_skip(&self) -> Option<Duration> {
        (self.auto_skip_seconds > 0).then(|| Duration::from_secs(self.auto_skip_seconds.into()))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    TopLeft,
    TopCenter,
    TopRight,
    Center,
    BottomLeft,
    BottomCenter,
    #[default]
    BottomRight,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_bottom_right() {
        assert_eq!(Config::default().popup.position, Position::BottomRight);
    }

    #[test]
    fn parses_kebab_case() {
        let c: Config = toml::from_str("[popup]\nposition = \"top-center\"").unwrap();
        assert_eq!(c.popup.position, Position::TopCenter);
    }

    #[test]
    fn empty_file_gives_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.popup.position, Position::BottomRight);
    }

    #[test]
    fn auto_skip_defaults_to_30_seconds() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.popup.auto_skip(), Some(Duration::from_secs(30)));
    }

    #[test]
    fn auto_skip_zero_turns_it_off() {
        let c: Config = toml::from_str("[popup]\nauto_skip_seconds = 0").unwrap();
        assert_eq!(c.popup.auto_skip(), None);
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(toml::from_str::<Config>("[popup]\npositon = \"center\"").is_err());
    }
}
