// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use anyhow::Result;
use traykit::Ui;
use traykit::dialog::{AnyId, Bounds, Dialog, EditStyle, Event, Flow, Id, Outcome, kind};

use crate::config_store::{Config, Position, Preset};
use crate::study::questions::{self, Question};
use crate::ui::popup::placement;

const TITLE: &str = "jik Settings";
const MAX_AUTO_SKIP: u32 = 3600;
const LABEL_LEN: usize = 60;
const CUSTOM_POSITION: &str = "Custom";
/// The popup size that the placement picker shows: a short question with four choices.
const POPUP_SIZE: (i32, i32) = (360, 300);

pub struct Edited {
    pub config: Config,
    pub autostart: bool,
    pub questions: Vec<Question>,
}

pub enum SettingsOutcome {
    Saved(Edited),
    Cancelled,
    Quit,
}

/// A question while the user edits it. It can be incomplete until the user saves.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    /// `None` for a new question. Existing questions keep their id, so their progress stays.
    id: Option<String>,
    text: String,
    choices: Vec<String>,
    answer: Option<String>,
}

impl Draft {
    fn new() -> Self {
        Self {
            id: None,
            text: String::new(),
            choices: Vec::new(),
            answer: None,
        }
    }

    fn from_question(q: &Question) -> Self {
        Self {
            id: Some(q.id.clone()),
            text: q.text.clone(),
            choices: q.choices.clone(),
            answer: Some(q.answer.clone()),
        }
    }

    fn label(&self) -> String {
        let first = self.text.lines().next().unwrap_or("").trim();
        if first.is_empty() {
            return "(new question)".into();
        }
        let mut label: String = first.chars().take(LABEL_LEN).collect();
        if first.chars().count() > LABEL_LEN {
            label.push_str("...");
        }
        label
    }

    fn answer_index(&self) -> Option<usize> {
        let answer = self.answer.as_ref()?;
        self.choices.iter().position(|c| c == answer)
    }
}

fn parse_choices(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect()
}

fn parse_auto_skip(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() {
        return Some(0);
    }
    text.parse().ok().filter(|&v| v <= MAX_AUTO_SKIP)
}

/// A problem in one question, or in the list as a whole.
struct Invalid {
    question: Option<usize>,
    message: String,
}

/// Make the questions to save. New questions get an id from their text.
fn build_questions(drafts: &[Draft]) -> Result<Vec<Question>, Invalid> {
    if drafts.is_empty() {
        return Err(Invalid {
            question: None,
            message: "Add at least one question.".into(),
        });
    }
    let mut taken: Vec<String> = drafts.iter().filter_map(|d| d.id.clone()).collect();
    let mut built = Vec::with_capacity(drafts.len());
    for (i, draft) in drafts.iter().enumerate() {
        let invalid = |message: String| Invalid {
            question: Some(i),
            message: format!("Question {}: {message}", i + 1),
        };
        if draft.text.trim().is_empty() {
            return Err(invalid("enter the question text.".into()));
        }
        if draft.choices.len() < 2 {
            return Err(invalid("enter at least two choices, one per line.".into()));
        }
        let Some(answer) = draft.answer.clone().filter(|a| draft.choices.contains(a)) else {
            return Err(invalid("select the correct answer.".into()));
        };
        let id = match &draft.id {
            Some(id) => id.clone(),
            None => {
                let id = questions::new_id(&draft.text, taken.iter().map(String::as_str));
                taken.push(id.clone());
                id
            }
        };
        let question = Question {
            id,
            text: draft.text.trim().to_string(),
            choices: draft.choices.clone(),
            answer,
        };
        if let Err(err) = questions::validate_all(std::slice::from_ref(&question)) {
            return Err(invalid(format!("{err}.")));
        }
        built.push(question);
    }
    questions::validate_all(&built).map_err(|err| Invalid {
        question: None,
        message: format!("{err}."),
    })?;
    Ok(built)
}

struct Ids {
    position: Id<kind::DropDown>,
    place: Id<kind::PlacementPicker>,
    auto_skip: Id<kind::Edit>,
    autostart: Id<kind::Check>,
    list: Id<kind::List>,
    add: Id<kind::Button>,
    delete: Id<kind::Button>,
    text: Id<kind::Edit>,
    choices: Id<kind::Edit>,
    answer: Id<kind::DropDown>,
    save: Id<kind::Button>,
    cancel: Id<kind::Button>,
}

fn build_dialog(ui: &Ui) -> Result<(Dialog, Ids)> {
    let mut d = Dialog::new(ui, TITLE, 720, 620)?;
    let b = Bounds::new;
    let one_line_digits = EditStyle {
        multiline: false,
        digits_only: true,
    };
    let multiline = EditStyle {
        multiline: true,
        digits_only: false,
    };
    d.add_group("General", b(16, 16, 688, 196))?;
    d.add_label("Popup position", b(32, 54, 216, 26))?;
    let position = d.add_drop_down(b(256, 54, 180, 26))?;
    d.add_label("Auto-skip after (seconds, 0 = off)", b(32, 88, 216, 26))?;
    let auto_skip = d.add_edit(one_line_digits, b(256, 88, 80, 26))?;
    let autostart = d.add_check("Start with Windows", b(32, 122, 300, 26))?;
    d.add_label(
        "Drag the popup in the picture to place it anywhere.",
        b(32, 168, 404, 26),
    )?;
    let (window_width, window_height) = POPUP_SIZE;
    let place = d.add_placement_picker(window_width, window_height, b(456, 46, 232, 156))?;

    d.add_group("Questions", b(16, 228, 688, 332))?;
    let list = d.add_list(b(32, 266, 260, 236))?;
    let add = d.add_button("Add", b(32, 514, 124, 30))?;
    let delete = d.add_button("Delete", b(168, 514, 124, 30))?;
    d.add_label("Question", b(308, 262, 380, 20))?;
    let text = d.add_edit(multiline, b(308, 284, 380, 56))?;
    d.add_label("Choices (one per line)", b(308, 348, 380, 20))?;
    let choices = d.add_edit(multiline, b(308, 370, 380, 96))?;
    d.add_label("Correct answer", b(308, 474, 380, 20))?;
    let answer = d.add_drop_down(b(308, 496, 380, 26))?;

    let save = d.add_button("Save", b(504, 574, 96, 30))?;
    let cancel = d.add_button("Cancel", b(608, 574, 96, 30))?;
    Ok((
        d,
        Ids {
            position,
            place,
            auto_skip,
            autostart,
            list,
            add,
            delete,
            text,
            choices,
            answer,
            save,
            cancel,
        },
    ))
}

/// The position that the placement picker shows: a named place if it is on one.
fn picked_position(d: &Dialog, ids: &Ids) -> Position {
    let p = d.placement(ids.place);
    Position::from_fractions(p.x(), p.y())
}

/// Select the named place in the drop-down, or Custom.
fn show_position(d: &Dialog, ids: &Ids, position: Position) {
    let index = match position {
        Position::Named(preset) => Preset::ALL.iter().position(|&p| p == preset),
        Position::Custom { .. } => None,
    };
    d.set_selected(ids.position, Some(index.unwrap_or(Preset::ALL.len())));
}

struct Editor {
    ids: Ids,
    /// The config before the edit. Save changes only the values that the window shows.
    base: Config,
    drafts: Vec<Draft>,
    current: Option<usize>,
}

impl Editor {
    fn labels(&self) -> Vec<String> {
        self.drafts.iter().map(Draft::label).collect()
    }

    fn select(&mut self, d: &Dialog, index: Option<usize>) {
        self.current = index.filter(|&i| i < self.drafts.len());
        d.set_selected(self.ids.list, self.current);
        let editing = self.current.is_some();
        let editors: [AnyId; 4] = [
            self.ids.text.into(),
            self.ids.choices.into(),
            self.ids.answer.into(),
            self.ids.delete.into(),
        ];
        for id in editors {
            d.set_enabled(id, editing);
        }
        match self.current {
            Some(i) => {
                let draft = &self.drafts[i];
                d.set_text(self.ids.text, &draft.text.replace('\n', "\r\n"));
                d.set_text(self.ids.choices, &draft.choices.join("\r\n"));
            }
            None => {
                d.set_text(self.ids.text, "");
                d.set_text(self.ids.choices, "");
            }
        }
        self.refresh_answer(d);
    }

    fn refresh_answer(&self, d: &Dialog) {
        match self.current.map(|i| &self.drafts[i]) {
            Some(draft) => {
                d.set_items(self.ids.answer, &draft.choices);
                d.set_selected(self.ids.answer, draft.answer_index());
            }
            None => d.set_items::<_, &str>(self.ids.answer, &[]),
        }
    }

    fn handle(&mut self, d: &mut Dialog, event: Event) -> Option<SettingsOutcome> {
        let ids = &self.ids;
        match event {
            Event::Changed(id) if id == ids.position => {
                // A named place moves the popup in the picture; Custom leaves it where it is.
                if let Some(preset) = d.selected(ids.position).and_then(|i| Preset::ALL.get(i)) {
                    d.set_placement(ids.place, placement(Position::Named(*preset)));
                }
            }
            Event::Changed(id) if id == ids.place => {
                show_position(d, ids, picked_position(d, ids));
            }
            Event::Changed(id) if id == ids.list => {
                let index = d.selected(ids.list);
                self.select(d, index);
            }
            Event::Changed(id) if id == ids.text => {
                if let Some(i) = self.current {
                    self.drafts[i].text = d.text(ids.text).replace("\r\n", "\n");
                    d.set_item(ids.list, i, &self.drafts[i].label());
                }
            }
            Event::Changed(id) if id == ids.choices => {
                if let Some(i) = self.current {
                    let draft = &mut self.drafts[i];
                    draft.choices = parse_choices(&d.text(ids.choices));
                    if draft.answer_index().is_none() {
                        draft.answer = None;
                    }
                    self.refresh_answer(d);
                }
            }
            Event::Changed(id) if id == ids.answer => {
                if let (Some(i), Some(choice)) = (self.current, d.selected(ids.answer)) {
                    let draft = &mut self.drafts[i];
                    draft.answer = draft.choices.get(choice).cloned();
                }
            }
            Event::Clicked(id) if id == ids.add => {
                self.drafts.push(Draft::new());
                d.set_items(ids.list, &self.labels());
                self.select(d, Some(self.drafts.len() - 1));
                d.focus(self.ids.text);
            }
            Event::Clicked(id) if id == ids.delete => {
                if let Some(i) = self.current {
                    self.drafts.remove(i);
                    d.set_items(ids.list, &self.labels());
                    let next = (!self.drafts.is_empty()).then(|| i.min(self.drafts.len() - 1));
                    self.select(d, next);
                }
            }
            Event::Clicked(id) if id == ids.save => return self.save(d),
            Event::Submit => return self.save(d),
            Event::Clicked(id) if id == ids.cancel => return Some(SettingsOutcome::Cancelled),
            Event::Close => return Some(SettingsOutcome::Cancelled),
            _ => {}
        }
        None
    }

    fn save(&mut self, d: &Dialog) -> Option<SettingsOutcome> {
        let ids = &self.ids;
        let Some(auto_skip) = parse_auto_skip(&d.text(ids.auto_skip)) else {
            d.warn(
                TITLE,
                &format!("Auto-skip must be a whole number from 0 to {MAX_AUTO_SKIP}."),
            );
            d.focus(ids.auto_skip);
            return None;
        };
        let questions = match build_questions(&self.drafts) {
            Ok(questions) => questions,
            Err(invalid) => {
                d.warn(TITLE, &invalid.message);
                if let Some(i) = invalid.question {
                    self.select(d, Some(i));
                    d.focus(self.ids.text);
                }
                return None;
            }
        };
        let mut config = self.base.clone();
        config.popup.position = picked_position(d, &self.ids);
        config.popup.auto_skip_seconds = auto_skip;
        Some(SettingsOutcome::Saved(Edited {
            config,
            autostart: d.checked(self.ids.autostart),
            questions,
        }))
    }
}

/// Show the settings window. It returns when the user saves or cancels.
pub fn edit(
    ui: &Ui,
    config: &Config,
    autostart: bool,
    questions: &[Question],
) -> Result<SettingsOutcome> {
    let (mut dialog, ids) = build_dialog(ui)?;
    let mut labels: Vec<&str> = Preset::ALL.iter().map(|p| p.label()).collect();
    labels.push(CUSTOM_POSITION);
    dialog.set_items(ids.position, &labels);
    dialog.set_placement(ids.place, placement(config.popup.position));
    show_position(&dialog, &ids, config.popup.position);
    dialog.set_text(ids.auto_skip, &config.popup.auto_skip_seconds.to_string());
    dialog.set_checked(ids.autostart, autostart);

    let mut editor = Editor {
        ids,
        base: config.clone(),
        drafts: questions.iter().map(Draft::from_question).collect(),
        current: None,
    };
    dialog.set_items(editor.ids.list, &editor.labels());
    let first = (!editor.drafts.is_empty()).then_some(0);
    editor.select(&dialog, first);

    let mut outcome = SettingsOutcome::Cancelled;
    let closed = dialog.run(|d, event| match editor.handle(d, event) {
        Some(result) => {
            outcome = result;
            Flow::Close
        }
        None => Flow::Continue,
    })?;
    Ok(match closed {
        Outcome::Closed => outcome,
        Outcome::Quit => SettingsOutcome::Quit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(text: &str, choices: &[&str], answer: Option<&str>) -> Draft {
        Draft {
            id: None,
            text: text.into(),
            choices: choices.iter().map(|c| c.to_string()).collect(),
            answer: answer.map(String::from),
        }
    }

    #[test]
    fn choices_are_trimmed_lines_without_blanks() {
        assert_eq!(parse_choices(" a \r\n\r\nb\n  \n c"), ["a", "b", "c"]);
    }

    #[test]
    fn auto_skip_must_be_in_range() {
        assert_eq!(parse_auto_skip(""), Some(0));
        assert_eq!(parse_auto_skip(" 45 "), Some(45));
        assert_eq!(parse_auto_skip("3600"), Some(3600));
        assert_eq!(parse_auto_skip("3601"), None);
        assert_eq!(parse_auto_skip("abc"), None);
    }

    #[test]
    fn label_is_the_first_line_and_short() {
        assert_eq!(draft("", &[], None).label(), "(new question)");
        assert_eq!(draft("Hi\nthere", &[], None).label(), "Hi");
        let long = "x".repeat(100);
        assert_eq!(draft(&long, &[], None).label().len(), LABEL_LEN + 3);
    }

    #[test]
    fn new_questions_get_unique_ids_and_old_ids_stay() {
        let mut old = draft("Capital?", &["A", "B"], Some("A"));
        old.id = Some("capital".into());
        let new = draft("Capital", &["C", "D"], Some("D"));
        let built = build_questions(&[old, new]).ok().unwrap();
        assert_eq!(built[0].id, "capital");
        assert_eq!(built[1].id, "capital-2");
        assert_eq!(built[1].answer, "D");
    }

    #[test]
    fn invalid_questions_name_the_question() {
        let good = draft("Q", &["A", "B"], Some("A"));
        let cases = [
            (draft("  ", &["A", "B"], Some("A")), "question text"),
            (draft("Q", &["A"], Some("A")), "two choices"),
            (draft("Q", &["A", "B"], None), "correct answer"),
            (draft("Q", &["A", "B"], Some("C")), "correct answer"),
        ];
        for (bad, expected) in cases {
            let Err(invalid) = build_questions(&[good.clone(), bad]) else {
                panic!("expected an error for {expected}");
            };
            assert_eq!(invalid.question, Some(1));
            assert!(
                invalid.message.starts_with("Question 2:"),
                "{}",
                invalid.message
            );
            assert!(invalid.message.contains(expected), "{}", invalid.message);
        }
    }

    #[test]
    fn empty_list_cannot_be_saved() {
        let Err(invalid) = build_questions(&[]) else {
            panic!("expected an error");
        };
        assert_eq!(invalid.question, None);
    }
}
