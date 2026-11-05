// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::config::{Config, Position};
use crate::files::{with_line_endings_of, write_atomically};

const DEFAULT_CONFIG: &str = include_str!("default.toml");
const TIME_SCALE_VAR: &str = "JIK_TIME_SCALE";
/// 86400 changes one day to one second. A larger value gives no useful test.
const MAX_TIME_SCALE: f64 = 86_400.0;

#[derive(Debug, Clone)]
pub struct Settings {
    pub config: Config,
    pub time_scale: f64,
}

impl Settings {
    pub fn load(config_path: &Path) -> Result<Self> {
        Ok(Self {
            config: load_config(config_path)?,
            time_scale: time_scale_from_env()?,
        })
    }
}

/// Read `config.toml`. If it does not exist, write the default file first.
pub fn load_config(path: &Path) -> Result<Config> {
    if !path.exists() {
        std::fs::write(path, DEFAULT_CONFIG)
            .with_context(|| format!("writing default {}", path.display()))?;
        eprintln!("created default {}", path.display());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Change only the values in the file, so that its comments and layout stay.
pub fn save_config(path: &Path, config: &Config) -> Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DEFAULT_CONFIG.to_string(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let updated = update_config_text(&text, config)
        .with_context(|| format!("updating {}", path.display()))?;
    write_atomically(path, updated)
}

fn update_config_text(text: &str, config: &Config) -> Result<String> {
    let mut doc: toml_edit::DocumentMut = text.parse()?;
    let popup = &config.popup;
    doc["popup"]["position"] = match popup.position {
        Position::Named(preset) => toml_edit::value(preset.key()),
        Position::Custom { x, y } => {
            // f32 to f64 adds digits (0.3 becomes 0.30000001...), so round again.
            let round = |v: f32| (f64::from(v) * 1000.0).round() / 1000.0;
            let mut table = toml_edit::InlineTable::new();
            table.insert("x", round(x).into());
            table.insert("y", round(y).into());
            toml_edit::value(table)
        }
    };
    doc["popup"]["auto_skip_seconds"] = toml_edit::value(i64::from(popup.auto_skip_seconds));
    let updated = with_line_endings_of(text, &doc.to_string());
    toml::from_str::<Config>(&updated).context("the updated file is not a valid config")?;
    Ok(updated)
}

fn time_scale_from_env() -> Result<f64> {
    match std::env::var(TIME_SCALE_VAR) {
        Ok(v) => parse_time_scale(&v),
        Err(_) => Ok(1.0),
    }
}

fn parse_time_scale(value: &str) -> Result<f64> {
    let scale: f64 = value
        .parse()
        .with_context(|| format!("{TIME_SCALE_VAR} must be a number"))?;
    if !(1.0..=MAX_TIME_SCALE).contains(&scale) {
        bail!("{TIME_SCALE_VAR} must be from 1 to {MAX_TIME_SCALE}");
    }
    Ok(scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_store::Preset;

    #[test]
    fn default_file_parses_to_defaults() {
        let c: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(c.popup.position, Position::Named(Preset::BottomRight));
    }

    #[test]
    fn update_keeps_comments_and_changes_values() {
        let mut config: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        config.popup.position = Position::Named(Preset::TopLeft);
        config.popup.auto_skip_seconds = 0;
        let updated = update_config_text(DEFAULT_CONFIG, &config).unwrap();
        for line in DEFAULT_CONFIG.lines().filter(|l| l.starts_with('#')) {
            assert!(updated.contains(line), "lost comment: {line}");
        }
        let parsed: Config = toml::from_str(&updated).unwrap();
        assert_eq!(parsed.popup.position, Position::Named(Preset::TopLeft));
        assert_eq!(parsed.popup.auto_skip(), None);
    }

    #[test]
    fn update_keeps_crlf_line_endings() {
        let crlf = DEFAULT_CONFIG.replace("\r\n", "\n").replace('\n', "\r\n");
        let updated = update_config_text(&crlf, &Config::default()).unwrap();
        assert_eq!(
            updated.matches('\n').count(),
            updated.matches("\r\n").count()
        );
        let lf = DEFAULT_CONFIG.replace("\r\n", "\n");
        let updated = update_config_text(&lf, &Config::default()).unwrap();
        assert!(!updated.contains('\r'));
    }

    #[test]
    fn update_adds_missing_values() {
        let config = Config::default();
        let updated = update_config_text("# empty\n", &config).unwrap();
        let parsed: Config = toml::from_str(&updated).unwrap();
        assert_eq!(parsed.popup.position, Position::Named(Preset::BottomRight));
        assert_eq!(parsed.popup.auto_skip_seconds, 30);
    }

    #[test]
    fn time_scale_must_be_in_range() {
        assert_eq!(parse_time_scale("1").unwrap(), 1.0);
        assert_eq!(parse_time_scale("60").unwrap(), 60.0);
        assert_eq!(parse_time_scale("86400").unwrap(), 86_400.0);
        assert!(parse_time_scale("0").is_err());
        assert!(parse_time_scale("0.5").is_err());
        assert!(parse_time_scale("1e-20").is_err());
        assert!(parse_time_scale("86401").is_err());
        assert!(parse_time_scale("-1").is_err());
        assert!(parse_time_scale("NaN").is_err());
        assert!(parse_time_scale("inf").is_err());
        assert!(parse_time_scale("fast").is_err());
    }

    #[test]
    fn update_writes_a_custom_position_as_x_and_y() {
        let mut config = Config::default();
        config.popup.position = Position::Custom { x: 0.3, y: 0.125 };
        let updated = update_config_text(DEFAULT_CONFIG, &config).unwrap();
        assert!(
            updated.contains("position = { x = 0.3, y = 0.125 }"),
            "{updated}"
        );
        let parsed: Config = toml::from_str(&updated).unwrap();
        assert_eq!(parsed.popup.position, Position::Custom { x: 0.3, y: 0.125 });
    }
}
