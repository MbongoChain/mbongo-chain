//! `scripts/provider-containment-linux.sh run` forwards the network the
//! service signs for (S1-5, RFC 0006 §2) and nothing else changes.
//!
//! The launcher runs as root on Linux and ends in `exec setpriv … <service>`.
//! Here it runs unmodified with stand-ins first on `PATH`: `uname`, `id` and
//! `nft` answer as a prepared host would, and `setpriv` records the exact
//! argument vector it was given instead of starting anything. Unix only:
//! the launcher is a Linux deployment tool.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RUNNER_UID: &str = "1001";

struct Host {
    dir: PathBuf,
}

impl Host {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("mbongo-launcher-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["bin", "base", "cgroup/service"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        let stub = |name: &str, body: &str| {
            let path = dir.join("bin").join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        stub("uname", "echo Linux");
        // `id -u` is root; `id -u <account>` is the runner's uid.
        stub(
            "id",
            &format!("if [ $# -le 1 ]; then echo 0; else echo {RUNNER_UID}; fi"),
        );
        stub(
            "nft",
            &format!(
                "case \"$2\" in\n\
                 table) printf 'table inet mbongo_runner {{\\n meta skuid {RUNNER_UID} reject\\n hook input priority filter; policy drop;\\n}}\\n' ;;\n\
                 chain) printf 'chain input {{ policy drop; }}\\n' ;;\n\
                 esac"
            ),
        );
        // Records its arguments, one per line, and starts nothing.
        stub(
            "setpriv",
            &format!(
                "for a in \"$@\"; do printf '%s\\n' \"$a\"; done > '{}'",
                dir.join("argv").display()
            ),
        );
        // What `prepare` would have recorded for MBONGO_INBOUND_ALLOW=22.
        std::fs::write(
            dir.join("base/inbound.policy"),
            "allow=22\nchain input { policy drop; }\n",
        )
        .unwrap();
        Self { dir }
    }

    fn launch(&self, network: &[&str]) -> Output {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/provider-containment-linux.sh");
        let path = format!(
            "{}:{}",
            self.dir.join("bin").display(),
            std::env::var("PATH").unwrap()
        );
        Command::new("bash")
            .arg(script)
            .arg("run")
            .arg(self.dir.join("base"))
            .arg("/srv/mbongo/bin/provider_service")
            .arg("/srv/mbongo/provider.json")
            .args(network)
            .env("PATH", path)
            .env("MBONGO_CGROUP", self.dir.join("cgroup"))
            .env("MBONGO_INBOUND_ALLOW", "22")
            .env_remove("MBONGO_RUNNER_NETWORK")
            .output()
            .unwrap()
    }

    /// The argument vector `setpriv` received, if it was reached.
    fn argv(&self) -> Option<Vec<String>> {
        std::fs::read_to_string(self.dir.join("argv"))
            .ok()
            .map(|s| s.lines().map(str::to_owned).collect())
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The sandbox `run` applies, unchanged by S1-5.
const SANDBOX: [&str; 7] = [
    "--reuid=mbongo-provider",
    "--regid=mbongo-provider",
    "--clear-groups",
    "--bounding-set=-all,+setuid,+setgid",
    "--inh-caps=-all,+setuid,+setgid",
    "--ambient-caps=-all,+setuid,+setgid",
    "--no-new-privs",
];

fn assert_started(host: &Host, out: &Output, network: &[&str]) {
    assert!(
        out.status.success(),
        "launcher failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let argv = host.argv().expect("setpriv was not reached");
    let mut expected: Vec<String> = SANDBOX.iter().map(|s| (*s).to_owned()).collect();
    expected.extend(
        [
            "/srv/mbongo/bin/provider_service",
            "--config",
            "/srv/mbongo/provider.json",
        ]
        .iter()
        .chain(network)
        .map(|s| (*s).to_owned()),
    );
    assert_eq!(argv, expected, "the service's argument vector");
}

#[test]
fn dev_reaches_the_service() {
    let host = Host::new("dev");
    let out = host.launch(&["--dev"]);
    assert_started(&host, &out, &["--dev"]);
}

#[test]
fn a_genesis_file_reaches_the_service() {
    let host = Host::new("genesis");
    let out = host.launch(&["--genesis", "/srv/mbongo/genesis.json"]);
    assert_started(&host, &out, &["--genesis", "/srv/mbongo/genesis.json"]);
}

#[test]
fn no_network_is_refused_before_anything_starts() {
    let host = Host::new("none");
    let out = host.launch(&[]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("--dev|--genesis <file>"), "{err}");
    assert!(
        host.argv().is_none(),
        "setpriv was reached without a network"
    );
}
