// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use serde::Deserialize;

use super::schedule::ScheduleSettings;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub popup: PopupConfig,
    pub schedule: ScheduleSettings,
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

/// Where the popup shows: a named place, or `x` and `y` from 0 (left or top) to 1 (right or
/// bottom) of the free space on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(try_from = "toml::Value")]
pub enum Position {
    Named(Preset),
    Custom { x: f32, y: f32 },
}

impl Default for Position {
    fn default() -> Self {
        Position::Named(Preset::BottomRight)
    }
}

impl Position {
    pub fn fractions(self) -> (f32, f32) {
        match self {
            Position::Named(preset) => preset.fractions(),
            Position::Custom { x, y } => (x, y),
        }
    }

    /// A named place if `(x, y)` is one, else a custom position rounded to 0.001.
    pub fn from_fractions(x: f32, y: f32) -> Self {
        let round = |v: f32| (v.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;
        let (x, y) = (round(x), round(y));
        Preset::ALL
            .into_iter()
            .find(|p| p.fractions() == (x, y))
            .map_or(Position::Custom { x, y }, Position::Named)
    }
}

impl TryFrom<toml::Value> for Position {
    type Error = String;

    fn try_from(value: toml::Value) -> Result<Self, String> {
        match value {
            toml::Value::String(name) => Preset::ALL
                .into_iter()
                .find(|p| p.key() == name)
                .map(Position::Named)
                .ok_or_else(|| {
                    let names: Vec<&str> = Preset::ALL.iter().map(|p| p.key()).collect();
                    format!(
                        "unknown position {name:?}; use one of {}, or {{ x = 0.5, y = 0.5 }}",
                        names.join(", ")
                    )
                }),
            toml::Value::Table(table) => {
                let fraction = |key: &str| -> Result<f32, String> {
                    let value = table
                        .get(key)
                        .ok_or_else(|| format!("position needs {key}"))?;
                    let number = match value {
                        toml::Value::Float(f) => *f,
                        toml::Value::Integer(i) => *i as f64,
                        _ => return Err(format!("position {key} must be a number")),
                    };
                    if !(0.0..=1.0).contains(&number) {
                        return Err(format!("position {key} must be from 0 to 1"));
                    }
                    Ok(number as f32)
                };
                if let Some(other) = table.keys().find(|k| *k != "x" && *k != "y") {
                    return Err(format!("unknown position field {other:?}; use x and y"));
                }
                Ok(Position::Custom {
                    x: fraction("x")?,
                    y: fraction("y")?,
                })
            }
            _ => Err("position must be a name or { x = ..., y = ... }".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Preset {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    #[default]
    BottomRight,
}

impl Preset {
    pub const ALL: [Preset; 9] = [
        Preset::TopLeft,
        Preset::TopCenter,
        Preset::TopRight,
        Preset::CenterLeft,
        Preset::Center,
        Preset::CenterRight,
        Preset::BottomLeft,
        Preset::BottomCenter,
        Preset::BottomRight,
    ];

    /// The value in `config.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Preset::TopLeft => "top-left",
            Preset::TopCenter => "top-center",
            Preset::TopRight => "top-right",
            Preset::CenterLeft => "center-left",
            Preset::Center => "center",
            Preset::CenterRight => "center-right",
            Preset::BottomLeft => "bottom-left",
            Preset::BottomCenter => "bottom-center",
            Preset::BottomRight => "bottom-right",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Preset::TopLeft => "Top left",
            Preset::TopCenter => "Top center",
            Preset::TopRight => "Top right",
            Preset::CenterLeft => "Center left",
            Preset::Center => "Center",
            Preset::CenterRight => "Center right",
            Preset::BottomLeft => "Bottom left",
            Preset::BottomCenter => "Bottom center",
            Preset::BottomRight => "Bottom right",
        }
    }

    pub fn fractions(self) -> (f32, f32) {
        let index = Preset::ALL.iter().position(|&p| p == self).unwrap_or(8);
        let step = |i: usize| i as f32 * 0.5;
        (step(index % 3), step(index / 3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(text: &str) -> Result<Position, toml::de::Error> {
        toml::from_str::<Config>(&format!("[popup]\nposition = {text}")).map(|c| c.popup.position)
    }

    #[test]
    fn default_is_bottom_right() {
        assert_eq!(
            Config::default().popup.position,
            Position::Named(Preset::BottomRight)
        );
    }

    #[test]
    fn parses_names() {
        assert_eq!(
            position("\"top-center\"").unwrap(),
            Position::Named(Preset::TopCenter)
        );
        assert_eq!(
            position("\"center-left\"").unwrap(),
            Position::Named(Preset::CenterLeft)
        );
    }

    #[test]
    fn parses_custom_fractions() {
        assert_eq!(
            position("{ x = 0.25, y = 1 }").unwrap(),
            Position::Custom { x: 0.25, y: 1.0 }
        );
    }

    #[test]
    fn rejects_bad_positions_with_a_clear_message() {
        let message = |text| position(text).unwrap_err().to_string();
        assert!(message("\"middle\"").contains("unknown position"));
        assert!(message("{ x = 1.5, y = 0 }").contains("from 0 to 1"));
        assert!(message("{ x = 0.5 }").contains("needs y"));
        assert!(message("{ x = 0.5, y = 0.5, z = 1 }").contains("unknown position field"));
        assert!(message("3").contains("must be a name"));
    }

    #[test]
    fn empty_file_gives_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.popup.position, Position::default());
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
    fn every_preset_key_parses_to_itself() {
        for preset in Preset::ALL {
            let parsed = position(&format!("\"{}\"", preset.key())).unwrap();
            assert_eq!(parsed, Position::Named(preset));
        }
    }

    #[test]
    fn presets_are_a_three_by_three_grid() {
        assert_eq!(Preset::TopLeft.fractions(), (0.0, 0.0));
        assert_eq!(Preset::CenterRight.fractions(), (1.0, 0.5));
        assert_eq!(Preset::BottomCenter.fractions(), (0.5, 1.0));
        assert_eq!(Preset::BottomRight.fractions(), (1.0, 1.0));
    }

    #[test]
    fn fractions_on_a_preset_give_its_name() {
        assert_eq!(
            Position::from_fractions(0.5, 0.0),
            Position::Named(Preset::TopCenter)
        );
        assert_eq!(
            Position::from_fractions(0.3333, 0.9),
            Position::Custom { x: 0.333, y: 0.9 }
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(toml::from_str::<Config>("[popup]\npositon = \"center\"").is_err());
    }
}
