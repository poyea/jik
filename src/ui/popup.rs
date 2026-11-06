// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Local};
use glasskit::Ui;
use glasskit::placement::Placement;
use glasskit::popup::{ChoicePopup, Mark, Prompt, Reply, Response};

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
    /// Show the question and wait for the reply.
    fn ask(&mut self, question: &Question) -> Result<PopupReply>;
    /// Show the question and return at once. The event loop tells when the reply is ready.
    fn show(&mut self, question: &Question) -> Result<()>;
    /// The reply, once the user has answered or skipped. It closes the popup.
    fn take_reply(&mut self) -> Option<PopupReply>;
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

    fn prompt(&self, question: &Question) -> Prompt {
        let correct = question.correct_index();
        Prompt::new(&question.text, question.choices.iter().map(String::as_str))
            .marks(|clicked| {
                let mut marks = vec![(correct, Mark::Good)];
                if clicked != correct {
                    marks.push((clicked, Mark::Bad));
                }
                marks
            })
            .dismiss_label("Skip")
            .auto_dismiss(self.auto_skip)
    }
}

impl Popup for GlassPopup {
    fn configure(&mut self, config: &PopupConfig) {
        self.popup.set_placement(placement(config.position));
        self.auto_skip = config.auto_skip();
    }

    fn ask(&mut self, question: &Question) -> Result<PopupReply> {
        Ok(to_reply(self.popup.ask(&self.prompt(question))?))
    }

    fn show(&mut self, question: &Question) -> Result<()> {
        Ok(self.popup.show(&self.prompt(question))?)
    }

    fn take_reply(&mut self) -> Option<PopupReply> {
        self.popup.take_reply().map(to_reply)
    }
}

fn to_reply(reply: Reply) -> PopupReply {
    let result = match reply.response {
        Response::Choice(i) => PopupResult::Answered(i),
        Response::Dismissed => PopupResult::Skipped,
        Response::TimedOut => PopupResult::TimedOut,
        Response::Quit => PopupResult::Quit,
    };
    PopupReply {
        result,
        at: reply.at.into(),
    }
}

pub fn placement(position: Position) -> Placement {
    let (x, y) = position.fractions();
    Placement::new(x, y)
}
