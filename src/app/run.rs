// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use traykit::Ui;
use traykit::presence::{self, Busy};

use super::APP_NAME;
use super::session::{Asked, Session};
use super::settings::{self, Files, Opened};
use crate::config_store::Config;
use crate::study::schedule::ScheduleConfig;
use crate::ui::popup::Popup;
use crate::ui::tray::{Command, MenuState, Tray, Wake};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pause {
    Until(DateTime<Utc>),
    Indefinite,
}

pub fn run_tray<P: Popup>(
    ui: &Ui,
    mut session: Session<P>,
    mut config: Config,
    files: Files,
) -> Result<()> {
    let mut tray = Tray::new(ui)?;
    let min_gap = chrono::Duration::from_std(session.schedule.config().min_gap)?;
    let mut earliest = Utc::now() + min_gap;
    let mut pause: Option<Pause> = None;
    // A due question that waits because the user cannot see it now: why, and the next check.
    let mut held: Option<(Busy, DateTime<Utc>)> = None;
    let mut autostart = traykit::autostart::is_enabled(APP_NAME);
    traykit::process::set_efficiency_mode(true);

    loop {
        let now = Utc::now();
        if matches!(pause, Some(Pause::Until(t)) if t <= now) {
            pause = None;
        }
        let pop_at = match pause {
            Some(Pause::Indefinite) => None,
            _ => session.schedule.next_due().map(|(_, due)| {
                let mut at = due.max(earliest);
                if let Some(Pause::Until(t)) = pause {
                    at = at.max(t);
                }
                if let Some((_, check)) = held {
                    at = at.max(check);
                }
                at
            }),
        };

        tray.set_menu(MenuState {
            status: status_text(pause, pop_at, now, held.map(|(busy, _)| busy)),
            busy: false,
            paused: pause.is_some(),
            autostart,
        });

        let ask_now = match tray.wait(pop_at)? {
            Wake::Quit => return Ok(()),
            // A question that you ask for shows at once. A due question waits until you can see it.
            Wake::Command(Command::AskNow) => true,
            Wake::Due => match presence::busy() {
                Some(busy) => {
                    held = Some((busy, Utc::now() + retry(busy, session.schedule.config())));
                    false
                }
                None => true,
            },
            Wake::Unlocked => {
                // Give the user a moment after they come back, then show the waiting question.
                earliest = earliest.max(Utc::now() + min_gap);
                if matches!(held, Some((Busy::Away, _))) {
                    held = None;
                }
                false
            }
            Wake::Command(Command::Pause(duration)) => {
                pause = Some(match duration {
                    Some(d) => Pause::Until(Utc::now() + chrono::Duration::from_std(d)?),
                    None => Pause::Indefinite,
                });
                false
            }
            Wake::Command(Command::Resume) => {
                pause = None;
                false
            }
            Wake::Command(Command::ToggleAutostart) => {
                match settings::set_autostart(!autostart) {
                    Ok(()) => autostart = !autostart,
                    Err(err) => report(&err),
                }
                false
            }
            Wake::Command(Command::Settings) => {
                tray.set_busy(true);
                traykit::process::set_efficiency_mode(false);
                let opened = settings::open(ui, &files, &config, autostart, &session.questions);
                traykit::process::set_efficiency_mode(true);
                tray.set_busy(false);
                match opened {
                    Ok(Opened::Saved(edited)) => {
                        session.popup.configure(&edited.config.popup);
                        session.questions = edited.questions;
                        session.schedule.sync(&session.questions, Utc::now());
                        if let Err(err) = session.schedule.save() {
                            report(&err);
                        }
                        config = edited.config;
                        if edited.autostart != autostart {
                            match settings::set_autostart(edited.autostart) {
                                Ok(()) => autostart = edited.autostart,
                                Err(err) => report(&err),
                            }
                        }
                    }
                    Ok(Opened::Unchanged) => {}
                    Ok(Opened::Quit) => return Ok(()),
                    Err(err) => report(&err),
                }
                false
            }
        };

        if ask_now {
            tray.set_busy(true);
            traykit::process::set_efficiency_mode(false);
            let asked = session.ask_next();
            traykit::process::set_efficiency_mode(true);
            tray.set_busy(false);
            match asked {
                Ok(Asked::Quit) => return Ok(()),
                Ok(Asked::Done) => {}
                Err(err) => report(&err),
            }
            held = None;
            earliest = Utc::now() + min_gap;
        }
    }
}

/// How long a held question waits before the next check.
fn retry(busy: Busy, config: &ScheduleConfig) -> chrono::Duration {
    let wait = match busy {
        Busy::Away => config.away_retry,
        Busy::FullScreen | Busy::Presenting => config.busy_retry,
    };
    chrono::Duration::from_std(wait).unwrap_or_else(|_| chrono::Duration::minutes(1))
}

/// jik runs in the background, so an error in one question round must not stop it.
/// Show the error and continue. The next round tries again.
fn report(err: &anyhow::Error) {
    let message = format!("{err:#}");
    eprintln!("error: {message}");
    traykit::show_error(APP_NAME, &message);
}

fn status_text(
    pause: Option<Pause>,
    pop_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    held: Option<Busy>,
) -> String {
    match (pause, pop_at, held) {
        (Some(Pause::Indefinite), _, _) => "Paused".into(),
        (Some(Pause::Until(t)), _, _) => format!("Paused until {}", when(t, now)),
        (None, None, _) => "No questions".into(),
        (None, Some(_), Some(busy)) => format!("Question waiting: {}", waiting_reason(busy)),
        (None, Some(t), None) if t <= now => "Question due now".into(),
        (None, Some(t), None) => format!("Next question at {}", when(t, now)),
    }
}

fn waiting_reason(busy: Busy) -> &'static str {
    match busy {
        Busy::Away => "you are away",
        Busy::FullScreen => "full-screen app",
        Busy::Presenting => "you are presenting",
    }
}

/// Only the time if it is today, else also the day, because the top box waits about a week.
fn when(t: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let (t, now) = (t.with_timezone(&Local), now.with_timezone(&Local));
    let format = if t.date_naive() == now.date_naive() {
        "%H:%M"
    } else {
        "%a %d %b %H:%M"
    };
    t.format(format).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_today_has_no_day() {
        let now = Utc::now();
        assert_eq!(when(now, now).len(), "09:00".len());
    }

    #[test]
    fn time_on_another_day_has_the_day() {
        let now = Utc::now();
        let later = now + chrono::Duration::days(3);
        let text = when(later, now);
        assert_eq!(
            text,
            later
                .with_timezone(&Local)
                .format("%a %d %b %H:%M")
                .to_string()
        );
    }

    #[test]
    fn status_shows_pause_before_schedule() {
        let now = Utc::now();
        assert_eq!(
            status_text(Some(Pause::Indefinite), Some(now), now, None),
            "Paused"
        );
        assert_eq!(status_text(None, None, now, None), "No questions");
        assert_eq!(status_text(None, Some(now), now, None), "Question due now");
    }

    #[test]
    fn status_says_why_a_question_waits() {
        let now = Utc::now();
        assert_eq!(
            status_text(None, Some(now), now, Some(Busy::FullScreen)),
            "Question waiting: full-screen app"
        );
        assert_eq!(
            status_text(None, Some(now), now, Some(Busy::Away)),
            "Question waiting: you are away"
        );
        let paused = status_text(Some(Pause::Indefinite), Some(now), now, Some(Busy::Away));
        assert_eq!(paused, "Paused");
    }

    #[test]
    fn away_waits_longer_than_busy() {
        let config = ScheduleConfig::default();
        assert_eq!(
            retry(Busy::FullScreen, &config),
            chrono::Duration::minutes(1)
        );
        assert_eq!(
            retry(Busy::Presenting, &config),
            chrono::Duration::minutes(1)
        );
        assert_eq!(retry(Busy::Away, &config), chrono::Duration::minutes(5));
    }
}
