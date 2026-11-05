// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config_store;
mod files;
mod study;
mod ui;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::Utc;

use app::{Files, Opened, Session};
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
Usage: jik [--once | --settings]

  (no option)  Run in the system tray and ask questions on the Leitner schedule.
  --once       Ask the question due soonest right away, then exit.
  --settings   Open the settings window, then exit.
  --help       Show this help.

Environment:
  JIK_TIME_SCALE  Divide all schedule durations by this number (testing).";

fn main() {
    if let Err(err) = run() {
        let message = format!("{err:#}");
        eprintln!("error: {message}");
        traykit::show_error("jik", &message);
        std::process::exit(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Tray,
    Once,
    Settings,
}

fn run() -> Result<()> {
    let mode = match std::env::args().nth(1).as_deref() {
        None => Mode::Tray,
        Some("--once") => Mode::Once,
        Some("--settings") => Mode::Settings,
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return Ok(());
        }
        Some(other) => bail!("unknown option {other:?}\n\n{USAGE}"),
    };

    let Some(_instance) = traykit::single_instance("jik.SingleInstance")? else {
        bail!("jik is already running. Use its tray icon, or quit it first.");
    };

    let ui = traykit::Ui::new()?;
    let dir = app_dir()?;
    let files = Files {
        config: dir.join(CONFIG_FILE),
        questions: dir.join(QUESTIONS_FILE),
    };
    let settings = Settings::load(&files.config)?;

    let questions = questions::load_or_create(&files.questions)?;
    eprintln!(
        "loaded {} questions from {}",
        questions.len(),
        files.questions.display()
    );

    if mode == Mode::Settings {
        let autostart = traykit::autostart::is_enabled(app::APP_NAME);
        if let Opened::Saved(edited) =
            app::open_settings(&ui, &files, &settings.config, autostart, &questions)?
            && edited.autostart != autostart
        {
            app::set_autostart(edited.autostart)?;
        }
        return Ok(());
    }

    let mut schedule = Schedule::load(
        &dir.join(STATE_FILE),
        ScheduleConfig {
            periods: settings.config.schedule.boxes.clone(),
            jitter: settings.config.schedule.jitter,
            min_gap: settings.config.schedule.min_gap,
            ..ScheduleConfig::default()
        }
        .scaled(settings.time_scale),
    )?;
    schedule.sync(&questions, Utc::now());
    schedule.save()?;

    let session = Session {
        questions,
        schedule,
        store: AnswerStore::new(dir.join(ANSWERS_FILE)),
        popup: GlassPopup::new(&ui, &settings.config.popup)?,
        rng: fastrand::Rng::new(),
    };

    if mode == Mode::Once {
        let mut session = session;
        session.ask_next()?;
        Ok(())
    } else {
        app::run_tray(&ui, session, settings.config, files)
    }
}

fn app_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("locating the executable")?;
    exe.parent()
        .map(PathBuf::from)
        .context("executable has no parent directory")
}
