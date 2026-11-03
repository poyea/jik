// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

const MAX_ID_LEN: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    pub choices: Vec<String>,
    pub answer: String,
}

impl Question {
    pub fn correct_index(&self) -> usize {
        self.choices
            .iter()
            .position(|c| *c == self.answer)
            .expect("question was validated on load")
    }

    fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("a question has an empty id");
        }
        if self.text.trim().is_empty() {
            bail!("question {:?} has empty text", self.id);
        }
        if self.choices.iter().any(|c| c.trim().is_empty()) {
            bail!("question {:?} has an empty choice", self.id);
        }
        if self.choices.len() < 2 {
            bail!("question {:?} needs at least two choices", self.id);
        }
        let unique: HashSet<_> = self.choices.iter().collect();
        if unique.len() != self.choices.len() {
            bail!("question {:?} has duplicate choices", self.id);
        }
        if !self.choices.contains(&self.answer) {
            bail!(
                "question {:?}: answer {:?} is not one of its choices",
                self.id,
                self.answer
            );
        }
        Ok(())
    }
}

/// Make an id for a new question from its text, for example "what-is-the-capital-of-france".
/// The id is different from all ids in `taken`.
pub fn new_id<'a>(text: &str, taken: impl IntoIterator<Item = &'a str>) -> String {
    let taken: HashSet<&str> = taken.into_iter().collect();
    let mut base = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            base.push(c);
        } else if !base.is_empty() && !base.ends_with('-') {
            base.push('-');
        }
        if base.chars().count() >= MAX_ID_LEN {
            break;
        }
    }
    let base = base.trim_end_matches('-');
    let base = if base.is_empty() { "question" } else { base };
    if !taken.contains(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken.contains(id.as_str()))
        .expect("a free id exists")
}

pub fn validate_all(questions: &[Question]) -> Result<()> {
    if questions.is_empty() {
        bail!("no [[question]] entries found");
    }
    let mut ids = HashSet::new();
    for q in questions {
        q.validate()?;
        if !ids.insert(q.id.as_str()) {
            bail!("duplicate question id {:?}", q.id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(id: &str, choices: &[&str], answer: &str) -> Question {
        Question {
            id: id.into(),
            text: "?".into(),
            choices: choices.iter().map(|c| c.to_string()).collect(),
            answer: answer.into(),
        }
    }

    fn error(questions: &[Question]) -> String {
        validate_all(questions).unwrap_err().to_string()
    }

    #[test]
    fn accepts_valid_questions() {
        let qs = [question("a", &["x", "y"], "y")];
        validate_all(&qs).unwrap();
        assert_eq!(qs[0].correct_index(), 1);
    }

    #[test]
    fn rejects_empty_list() {
        assert!(error(&[]).contains("no [[question]]"));
    }

    #[test]
    fn rejects_empty_id() {
        assert!(error(&[question(" ", &["x", "y"], "x")]).contains("empty id"));
    }

    #[test]
    fn rejects_empty_text() {
        let mut q = question("a", &["x", "y"], "x");
        q.text = "  ".into();
        assert!(error(&[q]).contains("empty text"));
    }

    #[test]
    fn rejects_empty_choice() {
        assert!(error(&[question("a", &["x", " "], "x")]).contains("empty choice"));
    }

    #[test]
    fn rejects_fewer_than_two_choices() {
        assert!(error(&[question("a", &["x"], "x")]).contains("at least two choices"));
    }

    #[test]
    fn rejects_duplicate_choices() {
        assert!(error(&[question("a", &["x", "x"], "x")]).contains("duplicate choices"));
    }

    #[test]
    fn rejects_answer_not_in_choices() {
        assert!(error(&[question("a", &["x", "y"], "z")]).contains("not one of its choices"));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let qs = [
            question("a", &["x", "y"], "x"),
            question("a", &["x", "y"], "y"),
        ];
        assert!(error(&qs).contains("duplicate question id"));
    }

    #[test]
    fn new_id_is_made_from_the_text() {
        assert_eq!(
            new_id("What is the capital of France?", []),
            "what-is-the-capital-of-france"
        );
        assert_eq!(new_id("  ?!  ", []), "question");
        assert_eq!(new_id("H2O", []), "h2o");
    }

    #[test]
    fn new_id_is_unique() {
        assert_eq!(new_id("Paris", ["paris"]), "paris-2");
        assert_eq!(new_id("Paris", ["paris", "paris-2"]), "paris-3");
    }

    #[test]
    fn new_id_is_short() {
        let id = new_id(&"word ".repeat(40), []);
        assert!(id.chars().count() <= MAX_ID_LEN);
        assert!(!id.ends_with('-'));
    }
}
