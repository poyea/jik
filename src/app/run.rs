// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use anyhow::Result;
use chrono::{DateTime, Local, Utc};

use super::session::{Asked, Session};
use crate::ui::popup::Popup;
use crate::ui::tray::{Command, MenuState, Tray, Wake};

const APP_NAME: &str = "jik";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pause {
    Until(DateTime<Utc>),
    Indefinite,
}

pub fn run_tray<P: Popup>(mut session: Session<P>) -> Result<()> {
    let mut tray = Tray::new()?;
    let min_gap = chrono::Duration::from_std(session.schedule.config().min_gap)?;
    let mut earliest = Utc::now() + min_gap;
    let mut pause: Option<Pause> = None;
    let mut autostart = winkit::autostart::is_enabled(APP_NAME);
    winkit::process::set_background(true);

    loop {
        let now = Utc::now();
        if matches!(pause, Some(Pause::Until(t)) if t <= now) {
            pause = None;
        }
        let pop_at = match pause {
            Some(Pause::Indefinite) => None,
            _ => session.schedule.next_due().map(|(_, due)| {
                let at = due.max(earliest);
                match pause {
                    Some(Pause::Until(t)) => at.max(t),
                    _ => at,
                }
            }),
        };

        tray.set_menu(MenuState {
            status: status_text(pause, pop_at, now),
            busy: false,
            paused: pause.is_some(),
            autostart,
        });

        let ask_now = match tray.wait(pop_at)? {
            Wake::Quit => return Ok(()),
            Wake::Due | Wake::Command(Command::AskNow) => true,
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
                match winkit::autostart::set_enabled(APP_NAME, !autostart) {
                    Ok(()) => autostart = !autostart,
                    Err(err) => report(&err.context("could not change Start with Windows")),
                }
                false
            }
        };

        if ask_now {
            tray.set_busy(true);
            winkit::process::set_background(false);
            let asked = session.ask_next();
            winkit::process::set_background(true);
            tray.set_busy(false);
            match asked {
                Ok(Asked::Quit) => return Ok(()),
                Ok(Asked::Done) => {}
                Err(err) => report(&err),
            }
            earliest = Utc::now() + min_gap;
        }
    }
}

/// jik runs in the background, so an error in one question round must not stop it.
/// Show the error and continue. The next round tries again.
fn report(err: &anyhow::Error) {
    let message = format!("{err:#}");
    eprintln!("error: {message}");
    winkit::show_error(APP_NAME, &message);
}

fn status_text(pause: Option<Pause>, pop_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    match (pause, pop_at) {
        (Some(Pause::Indefinite), _) => "Paused".into(),
        (Some(Pause::Until(t)), _) => format!("Paused until {}", when(t, now)),
        (None, None) => "No questions".into(),
        (None, Some(t)) if t <= now => "Question due now".into(),
        (None, Some(t)) => format!("Next question at {}", when(t, now)),
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
            status_text(Some(Pause::Indefinite), Some(now), now),
            "Paused"
        );
        assert_eq!(status_text(None, None, now), "No questions");
        assert_eq!(status_text(None, Some(now), now), "Question due now");
    }
}
