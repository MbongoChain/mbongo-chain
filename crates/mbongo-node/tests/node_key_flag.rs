//! `--node-key-file` as a real process sees it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mbongo-node-key-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Otherwise Unix refuses the file for its mode, before reading it.
fn make_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Runs the node on these paths, and fails rather than hangs if it does not stop.
fn run_node(key: &Path, data: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mbongo-node"))
        .arg("--node-key-file")
        .arg(key)
        .arg("--data-dir")
        .arg(data)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("the node kept running with a bad key file");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn a_malformed_key_file_stops_the_node_before_its_data_directory_is_touched() {
    let dir = scratch("malformed");
    let key = dir.join("node.key");
    std::fs::write(&key, "not a key").unwrap();
    make_private(&key);
    let data = dir.join("data");

    let output = run_node(&key, &data);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("expected 64 hex characters"), "{stderr}");
    assert!(
        !stderr.contains("not a key"),
        "the file's content was printed"
    );
    assert!(!data.exists(), "the data directory was created");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_missing_key_file_stops_the_node() {
    let dir = scratch("missing");
    let output = run_node(&dir.join("absent.key"), &dir.join("data"));
    assert!(!output.status.success());
    assert!(!dir.join("data").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
