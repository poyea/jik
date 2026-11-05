# traykit

Building blocks for small Windows tray apps, in Rust.

- **Tray**: an icon in the notification area with a menu of your own commands, and a wait loop that sleeps until a command, a deadline, or an unlock of the session.
- **Popup**: a frosted-glass popup with a question and choice buttons. It fades in without taking the keyboard focus, can show whether a choice was correct, and can dismiss itself after a countdown.
- **Dialog**: a small modal window of standard controls with typed handles, in the same colors.
- **Helpers**: start with Windows, single instance, efficiency mode, and whether the user can see a popup now.

All windows follow the Windows light or dark mode and the display scale.

## Example

```rust,no_run
use traykit::Ui;
use traykit::placement::Placement;
use traykit::popup::{ChoicePopup, Prompt};
use traykit::tray::{Icon, MenuItem, Tray, Wake};

#[derive(Clone)]
enum Command {
    Ask,
}

fn main() -> traykit::Result<()> {
    let ui = Ui::new()?;
    let size = Icon::tray_size();
    let icon = Icon::from_argb(size, vec![0xFF2E_86DE; (size * size) as usize])?;
    let mut tray = Tray::new(&ui, "Hello", &icon)?;
    tray.set_menu(&[
        MenuItem::command(Command::Ask, "Ask"),
        MenuItem::Quit("Quit".into()),
    ]);
    let mut popup = ChoicePopup::new(&ui, Placement::default())?;
    loop {
        match tray.wait(None)? {
            Wake::Command(Command::Ask) => {
                popup.ask(&Prompt::new("Is the tray working?", ["Yes", "No"]))?;
            }
            Wake::Quit => return Ok(()),
            _ => {}
        }
    }
}
```

A longer example, a small quiz app, is in `examples/quiz.rs`: `cargo run -p traykit --example quiz`.

## Requirements

- Windows 10 or 11.
- Rust 1.88 or later.

## License

MIT. See [LICENSE](LICENSE).
