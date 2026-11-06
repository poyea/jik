// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use anyhow::Result;
use chrono::{DateTime, Local, Utc};

use crate::study::answers::{AnswerRecord, AnswerStore, Outcome};
use crate::study::questions::Question;
use crate::study::schedule::Schedule;
use crate::ui::popup::{Popup, PopupReply, PopupResult};

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

/// A question that the popup shows now.
pub struct Shown {
    question_id: String,
    /// The choices as the popup shows them. An edit can change them while the popup shows.
    choices: Vec<String>,
    shown_at: DateTime<Local>,
}

impl<P: Popup> Session<P> {
    /// Show the question due soonest and wait for the reply.
    pub fn ask_next(&mut self) -> Result<Asked> {
        let Some(index) = self.due_index() else {
            return Ok(Asked::Done);
        };
        let question = &self.questions[index];
        let shown = Shown {
            question_id: question.id.clone(),
            choices: question.choices.clone(),
            shown_at: Local::now(),
        };
        let reply = self.popup.ask(question)?;
        self.finish(shown, reply)
    }

    /// Show the question due soonest and return at once. Give the reply of the popup to
    /// [`Session::finish`].
    pub fn show_next(&mut self) -> Result<Option<Shown>> {
        let Some(index) = self.due_index() else {
            return Ok(None);
        };
        let question = &self.questions[index];
        let shown = Shown {
            question_id: question.id.clone(),
            choices: question.choices.clone(),
            shown_at: Local::now(),
        };
        self.popup.show(question)?;
        Ok(Some(shown))
    }

    fn due_index(&self) -> Option<usize> {
        let (id, _) = self.schedule.next_due()?;
        Some(
            self.questions
                .iter()
                .position(|q| q.id == id)
                .expect("schedule is synced with the questions"),
        )
    }

    /// Record the reply. If the user deleted the question while it showed, there is nothing to
    /// record.
    pub fn finish(&mut self, shown: Shown, reply: PopupReply) -> Result<Asked> {
        let Some(question) = self.questions.iter().find(|q| q.id == shown.question_id) else {
            return Ok(Asked::Done);
        };
        let (choice, outcome) = match reply.result {
            PopupResult::Quit => return Ok(Asked::Quit),
            PopupResult::Skipped => (None, Outcome::Skipped),
            PopupResult::TimedOut => (None, Outcome::TimedOut),
            PopupResult::Answered(choice) => (Some(choice), Outcome::Answered),
        };
        // After an edit of the choices, the click is not an answer to the question as it is now.
        let (choice, outcome) = if choice.is_some() && question.choices != shown.choices {
            (None, Outcome::Skipped)
        } else {
            (choice, outcome)
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
            shown_at: shown.shown_at,
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
