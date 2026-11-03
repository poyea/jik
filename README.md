# jik

## Run

```sh
cargo run --release
```

jik runs in the system tray.
Use its menu to ask now, pause, start with Windows, open Settings, or quit.

In Settings you can change where the popup shows and when it skips a question, and add, edit, or delete your questions.
Changes apply at once.
`jik --settings` opens the same window without the tray.

jik keeps `questions.toml` (your questions), `config.toml` (settings), and `answers.jsonl` (your answers) next to the executable.
You can also edit the TOML files by hand and restart jik.
