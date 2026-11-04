// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use traykit::Ui;
use traykit::tray::{self, Icon, MenuItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    AskNow,
    Pause(Option<Duration>),
    Resume,
    ToggleAutostart,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    Due,
    /// The user unlocked the session.
    Unlocked,
    Command(Command),
    Quit,
}

#[derive(Debug, Clone, Default)]
pub struct MenuState {
    pub status: String,
    pub busy: bool,
    pub paused: bool,
    pub autostart: bool,
}

pub struct Tray {
    inner: tray::Tray<Command>,
    state: MenuState,
}

impl Tray {
    pub fn new(ui: &Ui) -> Result<Self> {
        Ok(Self {
            inner: tray::Tray::new(ui, "jik", &app_icon(Icon::tray_size())?)?,
            state: MenuState::default(),
        })
    }

    pub fn set_menu(&mut self, state: MenuState) {
        self.inner.set_tooltip(&format!("jik - {}", state.status));
        self.state = state;
        self.inner.set_menu(&menu_items(&self.state));
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.state.busy = busy;
        self.inner.set_menu(&menu_items(&self.state));
    }

    pub fn wait(&self, until: Option<DateTime<Utc>>) -> Result<Wake> {
        Ok(match self.inner.wait(until.map(Into::into))? {
            tray::Wake::Deadline => Wake::Due,
            tray::Wake::SessionUnlocked => Wake::Unlocked,
            tray::Wake::Quit => Wake::Quit,
            tray::Wake::Command(command) => Wake::Command(command),
        })
    }
}

fn menu_items(state: &MenuState) -> Vec<MenuItem<Command>> {
    let pause = if state.paused {
        MenuItem::command(Command::Resume, "Resume")
    } else {
        MenuItem::Submenu {
            label: "Pause".into(),
            items: vec![
                MenuItem::command(
                    Command::Pause(Some(Duration::from_secs(15 * 60))),
                    "15 minutes",
                ),
                MenuItem::command(Command::Pause(Some(Duration::from_secs(60 * 60))), "1 hour"),
                MenuItem::command(Command::Pause(None), "Until I resume"),
            ],
        }
    };
    vec![
        MenuItem::Label(state.status.clone()),
        MenuItem::Separator,
        MenuItem::command(Command::AskNow, "Ask now").enabled(!state.busy),
        pause,
        MenuItem::command(Command::ToggleAutostart, "Start with Windows").checked(state.autostart),
        MenuItem::command(Command::Settings, "Settings...").enabled(!state.busy),
        MenuItem::Separator,
        MenuItem::Quit("Quit".into()),
    ]
}

fn app_icon(size: u32) -> traykit::Result<Icon> {
    let center = size as f32 / 2.0;
    let petal_r = size as f32 * 0.46;
    let seed_r = size as f32 * 0.16;
    let mut pixels = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let d = (dx * dx + dy * dy).sqrt();
            let petal = (petal_r - d + 0.5).clamp(0.0, 1.0);
            let seed = (seed_r - d + 0.5).clamp(0.0, 1.0);
            let mix = |a: f32, b: f32| a * (1.0 - seed) + b * seed;
            let (r, g, b) = (mix(229.0, 45.0), mix(72.0, 25.0), mix(77.0, 30.0));
            let a = petal * 255.0;
            pixels.push(((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32);
        }
    }
    Icon::from_argb(size, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(items: &[MenuItem<Command>], wanted: Command) -> Option<&MenuItem<Command>> {
        items.iter().find_map(|item| match item {
            MenuItem::Command { command, .. } if *command == wanted => Some(item),
            MenuItem::Submenu { items, .. } => find(items, wanted),
            _ => None,
        })
    }

    #[test]
    fn pause_and_resume_swap() {
        let running = menu_items(&MenuState::default());
        assert!(find(&running, Command::Pause(None)).is_some());
        assert!(find(&running, Command::Resume).is_none());
        let paused = menu_items(&MenuState {
            paused: true,
            ..Default::default()
        });
        assert!(find(&paused, Command::Resume).is_some());
        assert!(find(&paused, Command::Pause(None)).is_none());
    }

    #[test]
    fn busy_disables_ask_now_and_settings() {
        let busy = menu_items(&MenuState {
            busy: true,
            ..Default::default()
        });
        for command in [Command::AskNow, Command::Settings] {
            assert!(matches!(
                find(&busy, command),
                Some(MenuItem::Command { enabled: false, .. })
            ));
        }
    }

    #[test]
    fn icon_has_the_asked_size() {
        assert_eq!(app_icon(20).unwrap().size(), 20);
    }
}
