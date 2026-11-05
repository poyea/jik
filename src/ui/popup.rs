// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Local};
use traykit::Ui;
use traykit::placement::Placement;
use traykit::popup::{ChoicePopup, Prompt, Response};

use crate::config_store::{PopupConfig, Position};
use crate::study::questions::Question;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupResult {
    Answered(usize),
    Skipped,
    TimedOut,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupReply {
    pub result: PopupResult,
    /// The time of the click or of the timeout.
    pub at: DateTime<Local>,
}

pub trait Popup {
    fn ask(&mut self, question: &Question) -> Result<PopupReply>;
    /// Use new settings from the next popup.
    fn configure(&mut self, config: &PopupConfig);
}

pub struct GlassPopup {
    popup: ChoicePopup,
    auto_skip: Option<Duration>,
}

impl GlassPopup {
    pub fn new(ui: &Ui, config: &PopupConfig) -> Result<Self> {
        Ok(Self {
            popup: ChoicePopup::new(ui, placement(config.position))?,
            auto_skip: config.auto_skip(),
        })
    }
}

impl Popup for GlassPopup {
    fn configure(&mut self, config: &PopupConfig) {
        self.popup.set_placement(placement(config.position));
        self.auto_skip = config.auto_skip();
    }

    fn ask(&mut self, question: &Question) -> Result<PopupReply> {
        let prompt = Prompt::new(&question.text, question.choices.iter().map(String::as_str))
            .correct(question.correct_index())
            .dismiss_label("Skip")
            .auto_dismiss(self.auto_skip);
        let reply = self.popup.ask(&prompt)?;
        let result = match reply.response {
            Response::Choice(i) => PopupResult::Answered(i),
            Response::Dismissed => PopupResult::Skipped,
            Response::TimedOut => PopupResult::TimedOut,
            Response::Quit => PopupResult::Quit,
        };
        Ok(PopupReply {
            result,
            at: reply.at.into(),
        })
    }
}

pub fn placement(position: Position) -> Placement {
    let (x, y) = position.fractions();
    Placement::new(x, y)
}
