// Purpose of this file
// ====================
// This is Cargo's build script. It runs automatically *before* compiling the
// main Rust code. We use it to:
//
//   1. Read the current git commit hash at build time
//   2. Generate a build date
//   3. Override CARGO_PKG_VERSION with a rich version string that includes
//      version + date + commit.
//
// Why do we override CARGO_PKG_VERSION?
// -------------------------------------
//   - It allows clap's built-in --version / -V to automatically show the
//     enriched string without any changes to src/tool.rs.

use std::process::Command;

fn main() {
    // Tell Cargo to re-run this build script whenever the git HEAD changes.
    // This ensures a new commit hash is baked in after every `git pull` / commit.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    // Get short git commit hash (12 characters is enough for traceability)
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string();

    // Record build date (UTC)
    let build_date = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Override Cargo's version for clap and env!("CARGO_PKG_VERSION")
    println!(
        "cargo:rustc-env=CARGO_PKG_VERSION={} built {} (commit {})",
        env!("CARGO_PKG_VERSION"), // base version from Cargo.toml
        build_date,
        commit
    );
}
