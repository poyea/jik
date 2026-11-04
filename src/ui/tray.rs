// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use traykit::Ui;
use traykit::tray::{self, IconImage, MenuItem};

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

const ID_ASK_NOW: u32 = 1;
const ID_PAUSE_15M: u32 = 2;
const ID_PAUSE_1H: u32 = 3;
const ID_PAUSE_INDEFINITE: u32 = 4;
const ID_RESUME: u32 = 5;
const ID_AUTOSTART: u32 = 6;
const ID_SETTINGS: u32 = 7;

pub struct Tray {
    inner: tray::Tray,
    state: MenuState,
}

impl Tray {
    pub fn new(ui: &Ui) -> Result<Self> {
        Ok(Self {
            inner: tray::Tray::new(ui, "jik", &app_icon(tray::icon_size()))?,
            state: MenuState::default(),
        })
    }

    pub fn set_menu(&mut self, state: MenuState) {
        self.inner.set_tooltip(&format!("jik - {}", state.status));
        self.state = state;
        self.inner.set_menu(menu_items(&self.state));
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.state.busy = busy;
        self.inner.set_menu(menu_items(&self.state));
    }

    pub fn wait(&self, until: Option<DateTime<Utc>>) -> Result<Wake> {
        Ok(match self.inner.wait(until.map(Into::into))? {
            tray::Wake::Deadline => Wake::Due,
            tray::Wake::SessionUnlocked => Wake::Unlocked,
            tray::Wake::Quit => Wake::Quit,
            tray::Wake::Command(id) => match command(id) {
                Some(c) => Wake::Command(c),
                None => return self.wait(until),
            },
        })
    }
}

fn command(id: u32) -> Option<Command> {
    Some(match id {
        ID_ASK_NOW => Command::AskNow,
        ID_PAUSE_15M => Command::Pause(Some(Duration::from_secs(15 * 60))),
        ID_PAUSE_1H => Command::Pause(Some(Duration::from_secs(60 * 60))),
        ID_PAUSE_INDEFINITE => Command::Pause(None),
        ID_RESUME => Command::Resume,
        ID_AUTOSTART => Command::ToggleAutostart,
        ID_SETTINGS => Command::Settings,
        _ => return None,
    })
}

fn menu_items(state: &MenuState) -> Vec<MenuItem> {
    let pause = if state.paused {
        MenuItem::command(ID_RESUME, "Resume")
    } else {
        MenuItem::Submenu {
            label: "Pause".into(),
            items: vec![
                MenuItem::command(ID_PAUSE_15M, "15 minutes"),
                MenuItem::command(ID_PAUSE_1H, "1 hour"),
                MenuItem::command(ID_PAUSE_INDEFINITE, "Until I resume"),
            ],
        }
    };
    vec![
        MenuItem::Label(state.status.clone()),
        MenuItem::Separator,
        MenuItem::Command {
            id: ID_ASK_NOW,
            label: "Ask now".into(),
            enabled: !state.busy,
            checked: false,
        },
        pause,
        MenuItem::Command {
            id: ID_AUTOSTART,
            label: "Start with Windows".into(),
            enabled: true,
            checked: state.autostart,
        },
        MenuItem::Command {
            id: ID_SETTINGS,
            label: "Settings...".into(),
            enabled: !state.busy,
            checked: false,
        },
        MenuItem::Separator,
        MenuItem::Quit("Quit".into()),
    ]
}

fn app_icon(size: u32) -> IconImage {
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
    IconImage { size, pixels }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_command_id_maps_to_a_command() {
        fn ids(items: &[MenuItem], out: &mut Vec<u32>) {
            for item in items {
                match item {
                    MenuItem::Command { id, .. } => out.push(*id),
                    MenuItem::Submenu { items, .. } => ids(items, out),
                    _ => {}
                }
            }
        }
        for paused in [false, true] {
            let mut found = Vec::new();
            ids(
                &menu_items(&MenuState {
                    paused,
                    ..Default::default()
                }),
                &mut found,
            );
            for id in found {
                assert!(command(id).is_some(), "menu id {id} has no command");
            }
        }
    }

    #[test]
    fn icon_has_one_pixel_per_cell() {
        let icon = app_icon(20);
        assert_eq!(icon.pixels.len(), 400);
    }
}
