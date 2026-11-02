// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::question::{Question, validate_all};

const SAMPLE: &str = include_str!("sample.toml");

#[derive(Deserialize)]
struct QuestionFile {
    #[serde(default, rename = "question")]
    questions: Vec<Question>,
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
