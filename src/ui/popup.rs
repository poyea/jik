// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use anyhow::Result;
use winkit::popup::{Anchor, ChoicePopup, Response};

use crate::config_store::Position;
use crate::study::questions::Question;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupResult {
    Answered(usize),
    Skipped,
    TimedOut,
    Quit,
}

pub trait Popup {
    fn ask(&mut self, question: &Question) -> Result<PopupResult>;
}

pub struct GlassPopup {
    popup: ChoicePopup,
    auto_skip: Option<Duration>,
}

impl GlassPopup {
    pub fn new(position: Position, auto_skip: Option<Duration>) -> Result<Self> {
        Ok(Self {
            popup: ChoicePopup::new(anchor(position))?,
            auto_skip,
        })
    }
}

impl Popup for GlassPopup {
    fn ask(&mut self, question: &Question) -> Result<PopupResult> {
        let response = self
            .popup
            .ask(&question.text, &question.choices, "Skip", self.auto_skip)?;
        Ok(match response {
            Response::Choice(i) => PopupResult::Answered(i),
            Response::Dismissed => PopupResult::Skipped,
            Response::TimedOut => PopupResult::TimedOut,
            Response::Quit => PopupResult::Quit,
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
