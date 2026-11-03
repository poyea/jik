// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use anyhow::Result;
use chrono::{Local, Utc};

use crate::study::answers::{AnswerRecord, AnswerStore, Outcome};
use crate::study::questions::Question;
use crate::study::schedule::Schedule;
use crate::ui::popup::{Popup, PopupResult};

pub struct Session<P: Popup> {
    pub questions: Vec<Question>,
    pub schedule: Schedule,
    pub store: AnswerStore,
    pub popup: P,
    pub rng: fastrand::Rng,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    Done,
    Quit,
}

impl<P: Popup> Session<P> {
    pub fn ask_next(&mut self) -> Result<Asked> {
        let Some((id, _)) = self.schedule.next_due() else {
            return Ok(Asked::Done);
        };
        let question = self
            .questions
            .iter()
            .find(|q| q.id == id)
            .expect("schedule is synced with the questions");

        let shown_at = Local::now();
        let reply = self.popup.ask(question)?;
        let (choice, outcome) = match reply.result {
            PopupResult::Quit => return Ok(Asked::Quit),
            PopupResult::Skipped => (None, Outcome::Skipped),
            PopupResult::TimedOut => (None, Outcome::TimedOut),
            PopupResult::Answered(choice) => (Some(choice), Outcome::Answered),
        };

        let correct = choice.map(|c| c == question.correct_index());
        let now = reply.at.with_timezone(&Utc);
        let moved = match correct {
            Some(correct) => self
                .schedule
                .record(&question.id, correct, now, &mut self.rng),
            None => self.schedule.skip(&question.id, now, &mut self.rng),
        }
        .expect("card exists for a synced question");
        self.schedule.save()?;
        self.store.append(&AnswerRecord {
            shown_at,
            answered_at: reply.at,
            question_id: question.id.clone(),
            question: question.text.clone(),
            choice: choice.map(|c| question.choices[c].clone()),
            correct,
            from_box: moved.from_box,
            to_box: moved.to_box,
            outcome,
        })?;
        let result = match (choice, correct) {
            (Some(c), Some(true)) => format!("{:?} (right)", question.choices[c]),
            (Some(c), _) => format!("{:?} (wrong)", question.choices[c]),
            (None, _) if outcome == Outcome::TimedOut => "timed out".into(),
            (None, _) => "skipped".into(),
        };
        eprintln!(
            "{}: {result}, box {} -> {}, next due {}",
            question.id,
            moved.from_box,
            moved.to_box,
            moved.due.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S"),
        );
        Ok(Asked::Done)
    }
}
