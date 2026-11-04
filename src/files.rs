// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Write the file in full or not at all: write a temporary file next to it, then replace the
/// file with it. Thus a crash or a full disk cannot leave half a file.
pub fn write_atomically(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    let tmp = temporary_path(path);
    std::fs::write(&tmp, contents).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(".tmp");
    PathBuf::from(name)
}

/// Use the line endings of `original` (CRLF or LF) in `text`, so a save does not change them all.
pub fn with_line_endings_of(original: &str, text: &str) -> String {
    let lf = text.replace("\r\n", "\n");
    if original.contains("\r\n") {
        lf.replace('\n', "\r\n")
    } else {
        lf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_file_is_next_to_the_file() {
        let tmp = temporary_path(Path::new("dir/config.toml"));
        assert_eq!(tmp, PathBuf::from("dir/config.toml.tmp"));
    }

    #[test]
    fn write_replaces_the_file_and_leaves_no_temporary_file() {
        let dir = std::env::temp_dir().join(format!("jik-files-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");
        write_atomically(&path, "old").unwrap();
        write_atomically(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(!temporary_path(&path).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn line_endings_follow_the_original() {
        assert_eq!(with_line_endings_of("a\r\nb", "x\ny\r\n"), "x\r\ny\r\n");
        assert_eq!(with_line_endings_of("a\nb", "x\r\ny\n"), "x\ny\n");
        assert_eq!(with_line_endings_of("", "x\r\n"), "x\n");
    }
}
