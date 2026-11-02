// Copyright (c) 2026 John Law
// SPDX-License-Identifier: MIT

use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("jik")).expect("unable to embed manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
