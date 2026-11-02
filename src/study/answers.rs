// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AnswerRecord {
    pub shown_at: DateTime<Local>,
    pub answered_at: DateTime<Local>,
    pub question_id: String,
    pub question: String,
    pub choice: Option<String>,
    pub correct: Option<bool>,
    pub from_box: usize,
    pub to_box: usize,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Answered,
    Skipped,
    TimedOut,
}

pub struct AnswerStore {
    path: PathBuf,
}

impl AnswerStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append(&self, record: &AnswerRecord) -> Result<()> {
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("opening {}", self.path.display()))?;
        file.write_all(line.as_bytes())
            .with_context(|| format!("writing {}", self.path.display()))
    }
}
