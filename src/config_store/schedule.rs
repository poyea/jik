// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use serde::Deserialize;

/// The `[schedule]` section of `config.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "ScheduleFile")]
pub struct ScheduleSettings {
    /// The wait of each Leitner box, from box 1 up.
    pub boxes: Vec<Duration>,
    /// The random change of each wait, as a fraction. 0.2 is plus or minus 20%.
    pub jitter: f64,
    /// The shortest time between two popups, and before the first popup after start.
    pub min_gap: Duration,
}

impl Default for ScheduleSettings {
    fn default() -> Self {
        const MIN: u64 = 60;
        const HOUR: u64 = 60 * MIN;
        const DAY: u64 = 24 * HOUR;
        Self {
            boxes: [10 * MIN, HOUR, DAY, 3 * DAY, 7 * DAY]
                .map(Duration::from_secs)
                .to_vec(),
            jitter: 0.2,
            min_gap: Duration::from_secs(2 * MIN),
        }
    }
}

/// The section as it is in the file, before the checks.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ScheduleFile {
    boxes: Vec<String>,
    jitter: f64,
    min_gap: String,
}

impl Default for ScheduleFile {
    fn default() -> Self {
        Self {
            boxes: ["10m", "1h", "1d", "3d", "7d"].map(String::from).to_vec(),
            jitter: 0.2,
            min_gap: "2m".into(),
        }
    }
}

impl TryFrom<ScheduleFile> for ScheduleSettings {
    type Error = String;

    fn try_from(file: ScheduleFile) -> Result<Self, String> {
        if file.boxes.is_empty() {
            return Err("schedule boxes needs at least one wait".into());
        }
        let boxes = file
            .boxes
            .iter()
            .map(|text| match parse_wait(text)? {
                wait if wait.is_zero() => {
                    Err(format!("schedule box wait {text:?} must be more than 0"))
                }
                wait => Ok(wait),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !(0.0..1.0).contains(&file.jitter) {
            return Err("schedule jitter must be from 0 to less than 1".into());
        }
        Ok(Self {
            boxes,
            jitter: file.jitter,
            min_gap: parse_wait(&file.min_gap)?,
        })
    }
}

/// A whole number and a unit: `s` (seconds), `m` (minutes), `h` (hours), `d` (days), or `w`
/// (weeks). For example `90s`, `10m`, or `3d`.
fn parse_wait(text: &str) -> Result<Duration, String> {
    let invalid =
        || format!("wait {text:?} must be a whole number and s, m, h, d, or w, like \"10m\"");
    let text = text.trim();
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .ok_or_else(invalid)?;
    let (number, unit) = text.split_at(split);
    let number: u64 = number.parse().map_err(|_| invalid())?;
    let unit_seconds: u64 = match unit.trim() {
        "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        "w" => 7 * 24 * 60 * 60,
        _ => return Err(invalid()),
    };
    number
        .checked_mul(unit_seconds)
        .map(Duration::from_secs)
        .ok_or_else(|| format!("wait {text:?} is too long"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(text: &str) -> Result<ScheduleSettings, toml::de::Error> {
        #[derive(Deserialize)]
        struct File {
            schedule: ScheduleSettings,
        }
        toml::from_str::<File>(&format!("[schedule]\n{text}")).map(|f| f.schedule)
    }

    #[test]
    fn waits_have_a_number_and_a_unit() {
        assert_eq!(parse_wait("90s"), Ok(Duration::from_secs(90)));
        assert_eq!(parse_wait("10m"), Ok(Duration::from_secs(600)));
        assert_eq!(parse_wait(" 2h "), Ok(Duration::from_secs(7200)));
        assert_eq!(parse_wait("3d"), Ok(Duration::from_secs(3 * 86_400)));
        assert_eq!(parse_wait("1w"), Ok(Duration::from_secs(7 * 86_400)));
        assert_eq!(parse_wait("0s"), Ok(Duration::ZERO));
        for bad in ["", "10", "m", "1.5h", "-1m", "10 minutes", "5y"] {
            assert!(parse_wait(bad).is_err(), "{bad:?} should fail");
        }
        assert!(parse_wait("99999999999999999w").is_err());
    }

    #[test]
    fn defaults_match_the_built_in_schedule() {
        let built_in = crate::study::schedule::ScheduleConfig::default();
        let settings = ScheduleSettings::default();
        assert_eq!(settings.boxes, built_in.periods);
        assert_eq!(settings.jitter, built_in.jitter);
        assert_eq!(settings.min_gap, built_in.min_gap);
    }

    #[test]
    fn empty_section_gives_the_defaults() {
        assert_eq!(schedule("").unwrap(), ScheduleSettings::default());
    }

    #[test]
    fn section_sets_each_value() {
        let s = schedule("boxes = [\"1m\", \"1h\"]\njitter = 0\nmin_gap = \"0s\"").unwrap();
        assert_eq!(
            s.boxes,
            [Duration::from_secs(60), Duration::from_secs(3600)]
        );
        assert_eq!(s.jitter, 0.0);
        assert_eq!(s.min_gap, Duration::ZERO);
    }

    #[test]
    fn bad_values_are_rejected() {
        for bad in [
            "boxes = []",
            "boxes = [\"0m\"]",
            "boxes = [\"ten\"]",
            "jitter = 1",
            "jitter = -0.1",
            "min_gap = \"soon\"",
            "unknown = 1",
        ] {
            assert!(schedule(bad).is_err(), "{bad:?} should fail");
        }
    }
}
