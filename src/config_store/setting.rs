// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::config::Config;

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
            config: load_or_create_config(config_path)?,
            time_scale: time_scale_from_env()?,
        })
    }
}

fn load_or_create_config(path: &Path) -> Result<Config> {
    if !path.exists() {
        std::fs::write(path, DEFAULT_CONFIG)
            .with_context(|| format!("writing default {}", path.display()))?;
        eprintln!("created default {}", path.display());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
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
    use crate::config_store::Position;

    #[test]
    fn default_file_parses_to_defaults() {
        let c: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(c.popup.position, Position::BottomRight);
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
}
