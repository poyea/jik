// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::question::{Question, validate_all};
use crate::files::{with_line_endings_of, write_atomically};

const SAMPLE: &str = include_str!("sample.toml");

#[derive(Deserialize, Serialize)]
struct QuestionFile {
    #[serde(default, rename = "question")]
    questions: Vec<Question>,
}

/// Write all questions. The comment at the top of the sample file stays at the top.
/// The file keeps its line endings (CRLF or LF).
pub fn save(path: &Path, questions: &[Question]) -> Result<()> {
    let old = std::fs::read_to_string(path).unwrap_or_default();
    let text = with_line_endings_of(&old, &to_text(questions)?);
    write_atomically(path, text)
}

fn to_text(questions: &[Question]) -> Result<String> {
    validate_all(questions)?;
    let header: String = SAMPLE
        .lines()
        .take_while(|line| line.starts_with('#'))
        .map(|line| format!("{line}\n"))
        .collect();
    let body = toml::to_string(&QuestionFile {
        questions: questions.to_vec(),
    })?;
    Ok(format!("{header}\n{body}"))
}

pub fn load_or_create(path: &Path) -> Result<Vec<Question>> {
    if !path.exists() {
        std::fs::write(path, SAMPLE)
            .with_context(|| format!("writing sample {}", path.display()))?;
        eprintln!("created sample {}", path.display());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse(&text).with_context(|| format!("loading {}", path.display()))
}

fn parse(text: &str) -> Result<Vec<Question>> {
    let file: QuestionFile = toml::from_str(text)?;
    validate_all(&file.questions)?;
    Ok(file.questions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_file_is_valid() {
        let qs = parse(SAMPLE).unwrap();
        assert!(qs.len() >= 2);
    }

    #[test]
    fn saved_text_loads_back_the_same_questions() {
        let qs = parse(SAMPLE).unwrap();
        let text = to_text(&qs).unwrap();
        assert!(text.starts_with('#'));
        assert_eq!(parse(&text).unwrap(), qs);
    }

    #[test]
    fn saving_invalid_questions_is_rejected() {
        let mut qs = parse(SAMPLE).unwrap();
        qs[0].answer = "not a choice".into();
        assert!(to_text(&qs).is_err());
    }

    #[test]
    fn file_without_questions_is_rejected() {
        assert!(parse("").is_err());
    }

    #[test]
    fn rules_apply_to_parsed_file() {
        let err = parse(
            r#"
            [[question]]
            id = "a"
            text = "?"
            choices = ["x", "y"]
            answer = "z"
            "#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("not one of its choices"));
    }
}
