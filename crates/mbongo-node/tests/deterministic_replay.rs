//! Deterministic replay integration test.
//!
//! Runs the `replay_harness` binary and asserts it exits with code 0.
//!
//! This test is marked `#[ignore]` because it spawns child processes and
//! requires the `mbongo-node` and `replay_harness` binaries to be built.
//! Run explicitly:
//!
//! ```bash
//! cargo build -p mbongo-node
//! cargo test -p mbongo-node --test deterministic_replay -- --ignored
//! ```

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

fn harness_binary_path() -> PathBuf {
    let self_exe = std::env::current_exe().expect("cannot determine own executable path");
    let dir = self_exe.parent().unwrap().parent().unwrap(); // tests are in deps/
    let name = if cfg!(windows) {
        "replay_harness.exe"
    } else {
        "replay_harness"
    };
    dir.join(name)
}

#[tokio::test]
#[ignore] // Requires pre-built binaries; run with: cargo test -- --ignored
async fn replay_harness_passes() {
    let binary = harness_binary_path();
    assert!(
        binary.exists(),
        "replay_harness binary not found at {binary:?}. Run `cargo build -p mbongo-node` first."
    );

    let output = tokio::time::timeout(
        Duration::from_secs(60),
        Command::new(&binary).stdout(Stdio::piped()).stderr(Stdio::piped()).output(),
    )
    .await
    .expect("replay harness timed out after 60s")
    .expect("failed to run replay harness");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("--- replay_harness stdout ---\n{stdout}");
    if !stderr.is_empty() {
        eprintln!("--- replay_harness stderr ---\n{stderr}");
    }

    assert!(
        output.status.success(),
        "replay_harness exited with non-zero status: {:?}",
        output.status
    );
    assert!(
        stdout.contains("DETERMINISTIC REPLAY: PASS"),
        "expected PASS in output"
    );
}
