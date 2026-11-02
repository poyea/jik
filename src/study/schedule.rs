// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::study::questions::Question;

#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    pub periods: Vec<Duration>,
    /// The jitter as a fraction of the period. For example, 0.2 is plus or minus 20%.
    pub jitter: f64,
    pub min_gap: Duration,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        const MIN: u64 = 60;
        const HOUR: u64 = 60 * MIN;
        const DAY: u64 = 24 * HOUR;
        Self {
            periods: [10 * MIN, HOUR, DAY, 3 * DAY, 7 * DAY]
                .map(Duration::from_secs)
                .to_vec(),
            jitter: 0.2,
            min_gap: Duration::from_secs(2 * MIN),
        }
    }
}

impl ScheduleConfig {
    pub fn scaled(self, scale: f64) -> Self {
        Self {
            periods: self.periods.iter().map(|p| p.div_f64(scale)).collect(),
            min_gap: self.min_gap.div_f64(scale),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    #[serde(rename = "box")]
    pub box_number: usize,
    pub due: DateTime<Utc>,
}

#[derive(Default, Serialize, Deserialize)]
struct StateFile {
    cards: BTreeMap<String, Card>,
}

pub struct Schedule {
    config: ScheduleConfig,
    cards: BTreeMap<String, Card>,
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moved {
    pub from_box: usize,
    pub to_box: usize,
    pub due: DateTime<Utc>,
}

impl Schedule {
    pub fn load(path: &Path, config: ScheduleConfig) -> Result<Self> {
        let cards = match std::fs::read_to_string(path) {
            Ok(text) => {
                serde_json::from_str::<StateFile>(&text)
                    .with_context(|| format!("parsing {}", path.display()))?
                    .cards
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Ok(Self {
            config,
            cards,
            path: path.to_path_buf(),
        })
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&StateFile {
            cards: self.cards.clone(),
        })?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, json).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("replacing {}", self.path.display()))
    }

    pub fn config(&self) -> &ScheduleConfig {
        &self.config
    }

    pub fn sync(&mut self, questions: &[Question], now: DateTime<Utc>) {
        self.cards
            .retain(|id, _| questions.iter().any(|q| q.id == *id));
        let top = self.config.periods.len();
        for (i, q) in questions.iter().enumerate() {
            let card = self.cards.entry(q.id.clone()).or_insert(Card {
                box_number: 1,
                // Add 1 ms for each card. Thus, cards with the same due time keep the file order.
                due: now + chrono::Duration::milliseconds(i as i64),
            });
            card.box_number = card.box_number.clamp(1, top);
        }
    }

    pub fn next_due(&self) -> Option<(&str, DateTime<Utc>)> {
        self.cards
            .iter()
            .min_by_key(|(id, card)| (card.due, id.as_str()))
            .map(|(id, card)| (id.as_str(), card.due))
    }

    #[cfg(test)]
    pub fn card(&self, id: &str) -> Option<Card> {
        self.cards.get(id).copied()
    }

    pub fn record(
        &mut self,
        id: &str,
        correct: bool,
        now: DateTime<Utc>,
        rng: &mut fastrand::Rng,
    ) -> Option<Moved> {
        let from_box = self.cards.get(id)?.box_number;
        let to_box = if correct {
            (from_box + 1).min(self.config.periods.len())
        } else {
            1
        };
        self.move_card(id, to_box, now, rng)
    }

    /// The card stays in its box and comes back after the period of that box.
    pub fn skip(&mut self, id: &str, now: DateTime<Utc>, rng: &mut fastrand::Rng) -> Option<Moved> {
        let box_number = self.cards.get(id)?.box_number;
        self.move_card(id, box_number, now, rng)
    }

    fn move_card(
        &mut self,
        id: &str,
        to_box: usize,
        now: DateTime<Utc>,
        rng: &mut fastrand::Rng,
    ) -> Option<Moved> {
        let factor = 1.0 + self.config.jitter * (rng.f64() * 2.0 - 1.0);
        let wait = self.config.periods[to_box - 1].mul_f64(factor.max(0.0));
        let due = chrono::Duration::from_std(wait)
            .ok()
            .and_then(|wait| now.checked_add_signed(wait))
            .unwrap_or(DateTime::<Utc>::MAX_UTC);

        let card = self.cards.get_mut(id)?;
        let from_box = card.box_number;
        *card = Card {
            box_number: to_box,
            due,
        };
        Some(Moved {
            from_box,
            to_box,
            due,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(id: &str) -> Question {
        Question {
            id: id.into(),
            text: "?".into(),
            choices: vec!["a".into(), "b".into()],
            answer: "a".into(),
        }
    }

    fn schedule() -> Schedule {
        Schedule {
            config: ScheduleConfig::default(),
            cards: BTreeMap::new(),
            path: PathBuf::new(),
        }
    }

    #[test]
    fn new_cards_start_in_box_1_in_file_order() {
        let now = Utc::now();
        let mut s = schedule();
        s.sync(&[question("b"), question("a")], now);
        assert_eq!(s.card("a").unwrap().box_number, 1);
        assert_eq!(s.next_due().unwrap().0, "b");
    }

    #[test]
    fn sync_drops_removed_questions() {
        let now = Utc::now();
        let mut s = schedule();
        s.sync(&[question("a"), question("b")], now);
        s.sync(&[question("b")], now);
        assert!(s.card("a").is_none());
        assert!(s.card("b").is_some());
    }

    #[test]
    fn right_moves_up_and_caps_wrong_resets() {
        let now = Utc::now();
        let mut rng = fastrand::Rng::with_seed(1);
        let mut s = schedule();
        s.sync(&[question("a")], now);
        let top = s.config.periods.len();
        for expected in 2..=top {
            assert_eq!(s.record("a", true, now, &mut rng).unwrap().to_box, expected);
        }
        assert_eq!(s.record("a", true, now, &mut rng).unwrap().to_box, top);
        let moved = s.record("a", false, now, &mut rng).unwrap();
        assert_eq!((moved.from_box, moved.to_box), (top, 1));
    }

    #[test]
    fn due_time_is_within_jitter() {
        let now = Utc::now();
        let mut rng = fastrand::Rng::with_seed(7);
        let mut s = schedule();
        s.sync(&[question("a")], now);
        let period = s.config.periods[1].as_secs_f64();
        for _ in 0..200 {
            s.cards.get_mut("a").unwrap().box_number = 1;
            let due = s.record("a", true, now, &mut rng).unwrap().due;
            let secs = (due - now).as_seconds_f64();
            assert!(secs >= period * 0.8 - 1e-6 && secs <= period * 1.2 + 1e-6);
        }
    }

    #[test]
    fn skip_keeps_box_and_delays_by_its_period() {
        let now = Utc::now();
        let mut rng = fastrand::Rng::with_seed(3);
        let mut s = schedule();
        s.sync(&[question("a")], now);
        s.record("a", true, now, &mut rng).unwrap();
        let moved = s.skip("a", now, &mut rng).unwrap();
        assert_eq!((moved.from_box, moved.to_box), (2, 2));
        let period = s.config.periods[1].as_secs_f64();
        let secs = (moved.due - now).as_seconds_f64();
        assert!(secs >= period * 0.8 - 1e-6 && secs <= period * 1.2 + 1e-6);
    }

    #[test]
    fn time_scale_divides_durations() {
        let c = ScheduleConfig::default().scaled(60.0);
        assert_eq!(c.periods[0], Duration::from_secs(10));
        assert_eq!(c.min_gap, Duration::from_secs(2));
    }
}
