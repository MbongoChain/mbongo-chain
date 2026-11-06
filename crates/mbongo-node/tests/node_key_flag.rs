//! `--node-key-file` as a real process sees it.

use std::path::PathBuf;
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mbongo-node-key-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_malformed_key_file_stops_the_node_before_its_data_directory_is_touched() {
    let dir = scratch("malformed");
    let key = dir.join("node.key");
    std::fs::write(&key, "not a key").unwrap();
    let data = dir.join("data");

    let output = Command::new(env!("CARGO_BIN_EXE_mbongo-node"))
        .arg("--node-key-file")
        .arg(&key)
        .arg("--data-dir")
        .arg(&data)
        .output()
        .unwrap();

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
    let output = Command::new(env!("CARGO_BIN_EXE_mbongo-node"))
        .arg("--node-key-file")
        .arg(dir.join("absent.key"))
        .arg("--data-dir")
        .arg(dir.join("data"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!dir.join("data").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
