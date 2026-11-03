// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Local};
use winkit::popup::{Anchor, ChoicePopup, Response};

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
    pub fn new(config: &PopupConfig) -> Result<Self> {
        Ok(Self {
            popup: ChoicePopup::new(anchor(config.position))?,
            auto_skip: config.auto_skip(),
        })
    }
}

impl Popup for GlassPopup {
    fn configure(&mut self, config: &PopupConfig) {
        self.popup.set_anchor(anchor(config.position));
        self.auto_skip = config.auto_skip();
    }

    fn ask(&mut self, question: &Question) -> Result<PopupReply> {
        let reply = self.popup.ask(
            &question.text,
            &question.choices,
            Some(question.correct_index()),
            "Skip",
            self.auto_skip,
        )?;
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

fn anchor(position: Position) -> Anchor {
    match position {
        Position::TopLeft => Anchor::TopLeft,
        Position::TopCenter => Anchor::TopCenter,
        Position::TopRight => Anchor::TopRight,
        Position::Center => Anchor::Center,
        Position::BottomLeft => Anchor::BottomLeft,
        Position::BottomCenter => Anchor::BottomCenter,
        Position::BottomRight => Anchor::BottomRight,
    }
}
