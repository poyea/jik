// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

//! A tray app that asks a question each minute and on "Ask now".
//!
//! Run it with `cargo run -p traykit --example quiz`.

use std::time::{Duration, SystemTime};

use traykit::Ui;
use traykit::placement::Placement;
use traykit::popup::{ChoicePopup, Prompt, Response};
use traykit::tray::{Icon, MenuItem, Tray, Wake};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    AskNow,
}

fn main() -> traykit::Result<()> {
    let ui = Ui::new()?;
    let size = Icon::tray_size();
    let icon = Icon::from_argb(size, vec![0xFF2E_86DE; (size * size) as usize])?;
    let mut tray = Tray::new(&ui, "Quiz", &icon)?;
    tray.set_menu(&[
        MenuItem::command(Command::AskNow, "Ask now"),
        MenuItem::Separator,
        MenuItem::Quit("Quit".into()),
    ]);
    let mut popup = ChoicePopup::new(&ui, Placement::default())?;
    loop {
        let next = SystemTime::now() + Duration::from_secs(60);
        match tray.wait(Some(next))? {
            Wake::Quit => return Ok(()),
            Wake::SessionUnlocked => {}
            Wake::Deadline | Wake::Command(Command::AskNow) => {
                let prompt = Prompt::new("What is 2 + 2?", ["3", "4", "5"])
                    .correct(1)
                    .auto_dismiss(Some(Duration::from_secs(30)));
                if popup.ask(&prompt)?.response == Response::Quit {
                    return Ok(());
                }
            }
        }
    }
}
