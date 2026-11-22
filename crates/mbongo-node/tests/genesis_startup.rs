//! `--dev` and `--genesis` as a real node process sees them (S1-4,
//! RFC 0006 §1): which genesis it starts from, that it refuses before any
//! network start, and that a data directory stays bound to its genesis
//! across restarts.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use mbongo_core::genesis::DEV_GENESIS_FILE;
use mbongo_core::{chain_id, Genesis};
use mbongo_storage::{RocksDbStorage, Storage};

// Fixed and outside the Linux ephemeral range (scripts/check-harness-ports.mjs).
// Only `starts_and_restarts_on_its_own_genesis` binds them, one node at a time.
const RPC_PORT: u16 = 24944;
const REST_PORT: u16 = 24080;
const P2P_PORT: u16 = 24333;

const MARKER: &str = "MBONGO_GENESIS";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mbongo-genesis-start-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn node(args: &[&str], data: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mbongo-node"));
    cmd.args(args)
        .args(["--rpc-port", &RPC_PORT.to_string()])
        .args(["--rest-port", &REST_PORT.to_string()])
        .args(["--p2p-port", &P2P_PORT.to_string()])
        .args(["--p2p-host", "127.0.0.1", "--no-mdns"])
        .arg("--data-dir")
        .arg(data)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// A node that must refuse to start: it has to exit on its own.
fn refused(args: &[&str], data: &Path) -> Output {
    let mut child = node(args, data).spawn().unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("the node kept running: {args:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success(), "{args:?} succeeded");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Refused before the network: no P2P identity was ever created.
    assert!(
        !stdout.contains("PeerId:"),
        "{args:?} reached the network:\n{stdout}"
    );
    output
}

/// A node that must start: returns its banner, up to the P2P line, then
/// stops it.
fn started(args: &[&str], data: &Path) -> String {
    let mut child = node(args, data).spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut banner = String::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                banner.push_str(&line);
                banner.push('\n');
                if line.contains("PeerId:") {
                    break;
                }
            }
            Err(_) => {
                let _ = child.kill();
                let output = child.wait_with_output().unwrap();
                panic!(
                    "{args:?} did not start:\n{banner}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
    child.kill().unwrap();
    child.wait().unwrap();
    banner
}

fn line<'a>(banner: &'a str, label: &str) -> &'a str {
    banner
        .lines()
        .find_map(|l| l.trim_start().strip_prefix(label))
        .unwrap_or_else(|| panic!("no {label} line in:\n{banner}"))
        .trim()
}

/// A private network: the dev authority, one funded account, timestamp 7.
const PRIVATE_GENESIS: &str = "{\"allocations\":[{\"address\":\"7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e\",\"amount\":\"5\"}],\"authorities\":[\"b1c2c08e2e372f831ca40e2a6e442ddc18f1dc1b6e0b39a0a32b41d6e5541c15\"],\"name\":\"private-1\",\"schema\":\"mbongo-genesis:v1\",\"timestamp\":7}";

#[test]
fn starts_and_restarts_on_its_own_genesis() {
    let dir = scratch("start");
    let net = dir.join("private.json");
    std::fs::write(&net, PRIVATE_GENESIS).unwrap();
    let net_arg = net.to_str().unwrap();
    let dev_id = chain_id(DEV_GENESIS_FILE).to_string();
    let net_id = chain_id(PRIVATE_GENESIS.as_bytes()).to_string();
    assert_ne!(dev_id, net_id);

    // --dev on a new directory, then twice again on the same one.
    let dev_data = dir.join("dev");
    let banner = started(&["--dev"], &dev_data);
    assert_eq!(line(&banner, "Chain id:"), dev_id);
    assert!(
        line(&banner, "Data dir:").ends_with("(new chain)"),
        "{banner}"
    );
    let marker = std::fs::read(dev_data.join(MARKER)).unwrap();
    for _ in 0..2 {
        let banner = started(&["--dev"], &dev_data);
        assert_eq!(line(&banner, "Chain id:"), dev_id);
        assert!(
            line(&banner, "Data dir:").ends_with("(existing chain)"),
            "{banner}"
        );
        assert_eq!(std::fs::read(dev_data.join(MARKER)).unwrap(), marker);
    }

    // --genesis on a new directory, and again.
    let net_data = dir.join("private");
    let banner = started(&["--genesis", net_arg], &net_data);
    assert_eq!(line(&banner, "Chain id:"), net_id);
    assert!(
        line(&banner, "Data dir:").ends_with("(new chain)"),
        "{banner}"
    );
    let banner = started(&["--genesis", net_arg], &net_data);
    assert!(
        line(&banner, "Data dir:").ends_with("(existing chain)"),
        "{banner}"
    );

    // The state each one was initialized with: the private network has its
    // allocation and none of the dev accounts.
    {
        let storage = RocksDbStorage::open(&net_data).unwrap();
        let private = Genesis::from_file_bytes(PRIVATE_GENESIS.as_bytes()).unwrap();
        let (address, amount) = private.allocations[0];
        assert_eq!(
            storage.get_account(&address).unwrap().unwrap().balance,
            amount
        );
        for (dev_address, _) in Genesis::dev().allocations {
            assert!(storage.get_account(&dev_address).unwrap().is_none());
        }
        assert_eq!(storage.get_latest_height().unwrap(), 0);
        let storage = RocksDbStorage::open(&dev_data).unwrap();
        for (address, amount) in Genesis::dev().allocations {
            assert_eq!(
                storage.get_account(&address).unwrap().unwrap().balance,
                amount
            );
        }
    }

    // Each directory refuses the other genesis, and is left as it was.
    let out = refused(&["--genesis", net_arg], &dev_data);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!("belongs to chain {dev_id}")),
        "{stderr}"
    );
    let out = refused(&["--dev"], &net_data);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&format!("belongs to chain {net_id}")),
        "{stderr}"
    );
    assert_eq!(std::fs::read(dev_data.join(MARKER)).unwrap(), marker);
    // Still usable with the right one.
    let banner = started(&["--dev"], &dev_data);
    assert!(
        line(&banner, "Data dir:").ends_with("(existing chain)"),
        "{banner}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_genesis_source_is_required_and_exclusive() {
    let dir = scratch("flags");
    let data = dir.join("data");
    for args in [&[][..], &["--dev", "--genesis", "x.json"][..]] {
        let out = refused(args, &data);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(!data.exists(), "{args:?} created the data directory");
    }
    let out = refused(&["--chain", "dev"], &data);
    assert!(String::from_utf8_lossy(&out.stderr).contains("--chain"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_invalid_or_missing_genesis_file_stops_the_node_first() {
    let dir = scratch("invalid");
    let data = dir.join("data");
    let cases = [
        ("missing.json", None, "cannot read genesis file"),
        ("newline.json", Some(format!("{PRIVATE_GENESIS}\n")), "invalid genesis file"),
        ("spaces.json", Some(PRIVATE_GENESIS.replace("\":", "\": ")), "invalid genesis file"),
        ("two-authorities.json", Some(PRIVATE_GENESIS.replace(
            "[\"b1c2c08e2e372f831ca40e2a6e442ddc18f1dc1b6e0b39a0a32b41d6e5541c15\"]",
            "[\"b1c2c08e2e372f831ca40e2a6e442ddc18f1dc1b6e0b39a0a32b41d6e5541c15\",\"b1c2c08e2e372f831ca40e2a6e442ddc18f1dc1b6e0b39a0a32b41d6e5541c15\"]",
        )), "invalid genesis file"),
        ("empty.json", Some(String::new()), "invalid genesis file"),
    ];
    assert_eq!(cases.len(), 5);
    for (name, content, expected) in cases {
        let path = dir.join(name);
        if let Some(content) = content {
            std::fs::write(&path, content).unwrap();
        }
        let out = refused(&["--genesis", path.to_str().unwrap()], &data);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(expected), "{name}: {stderr}");
        assert!(!data.exists(), "{name}: the data directory was created");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_directory_without_a_marker_is_refused_unchanged() {
    // A database written without a marker, as v0.4 did: it is never
    // opened, so even its RocksDB files stay byte-identical.
    let dir = scratch("nomarker");
    let data = dir.join("data");
    {
        let storage = RocksDbStorage::open(&data).unwrap();
        storage.put_block_height_index(0, mbongo_core::Hash([1u8; 32])).unwrap();
    }
    let snapshot = |d: &Path| -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file())
            .map(|p| {
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    std::fs::read(&p).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };
    let before = snapshot(&data);
    let out = refused(&["--dev"], &data);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no MBONGO_GENESIS marker"), "{stderr}");
    assert_eq!(snapshot(&data), before);
    assert!(!data.join(MARKER).exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
