// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config_store;
mod study;
mod ui;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::Utc;

use app::Session;
use config_store::Settings;
use study::answers::AnswerStore;
use study::questions;
use study::schedule::{Schedule, ScheduleConfig};
use ui::popup::GlassPopup;

const CONFIG_FILE: &str = "config.toml";
const QUESTIONS_FILE: &str = "questions.toml";
const ANSWERS_FILE: &str = "answers.jsonl";
const STATE_FILE: &str = "state.json";

const USAGE: &str = "\
Usage: jik [--once]

  (no option)  Run in the system tray and ask questions on the Leitner schedule.
  --once       Ask the question due soonest right away, then exit.
  --help       Show this help.

Environment:
  JIK_TIME_SCALE  Divide all schedule durations by this number (testing).";

fn main() {
    if let Err(err) = run() {
        let message = format!("{err:#}");
        eprintln!("error: {message}");
        winkit::show_error("jik", &message);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let once = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--once") => true,
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return Ok(());
        }
        Some(other) => bail!("unknown option {other:?}\n\n{USAGE}"),
    };

    let Some(_instance) = winkit::single_instance("jik.SingleInstance")? else {
        bail!("jik is already running. Use its tray icon, or quit it first.");
    };

    let dir = app_dir()?;
    let settings = Settings::load(&dir.join(CONFIG_FILE))?;

    let questions_path = dir.join(QUESTIONS_FILE);
    let questions = questions::load_or_create(&questions_path)?;
    eprintln!(
        "loaded {} questions from {}",
        questions.len(),
        questions_path.display()
    );

    let mut schedule = Schedule::load(
        &dir.join(STATE_FILE),
        ScheduleConfig::default().scaled(settings.time_scale),
    )?;
    schedule.sync(&questions, Utc::now());
    schedule.save()?;

    let session = Session {
        questions,
        schedule,
        store: AnswerStore::new(dir.join(ANSWERS_FILE)),
        popup: GlassPopup::new(
            settings.config.popup.position,
            settings.config.popup.auto_skip(),
        )?,
        rng: fastrand::Rng::new(),
    };

    if once {
        let mut session = session;
        session.ask_next()?;
        Ok(())
    } else {
        app::run_tray(session)
    }
}

fn app_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("locating the executable")?;
    exe.parent()
        .map(PathBuf::from)
        .context("executable has no parent directory")
}
