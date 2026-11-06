//! A stable P2P identity, read from a file.
//!
//! The file holds a 32-byte Ed25519 seed as 64 hex characters. It is the
//! node's network identity, and is not an authority or account key.

use std::path::Path;

use libp2p::identity::Keypair;

/// Reads a node key file.
///
/// # Errors
///
/// Returns a description if the file cannot be read, does not hold exactly 64
/// hex characters, or on Unix is readable by anyone but its owner.
pub fn load_node_key(path: &Path) -> Result<Keypair, String> {
    let shown = path.display();
    check_private(path)?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("node key file {shown}: {e}"))?;
    parse_node_key(text.trim()).map_err(|e| format!("node key file {shown}: {e}"))
}

/// Builds a keypair from 64 hex characters.
fn parse_node_key(text: &str) -> Result<Keypair, String> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("expected 64 hex characters".to_string());
    }
    let mut seed = [0u8; 32];
    for (index, byte) in seed.iter_mut().enumerate() {
        let pair = &text[index * 2..index * 2 + 2];
        *byte =
            u8::from_str_radix(pair, 16).map_err(|_| "expected 64 hex characters".to_string())?;
    }
    // Taken by reference: the library wipes the buffer it is given.
    Keypair::ed25519_from_bytes(&mut seed).map_err(|e| e.to_string())
}

#[cfg(unix)]
fn check_private(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let shown = path.display();
    let mode = std::fs::metadata(path)
        .map_err(|e| format!("node key file {shown}: {e}"))?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        return Err(format!(
            "node key file {shown}: readable by others, expected mode 600"
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn check_private(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    #[test]
    fn the_same_seed_gives_the_same_identity() {
        let a = parse_node_key(SEED).unwrap().public().to_peer_id();
        let b = parse_node_key(SEED).unwrap().public().to_peer_id();
        assert_eq!(a, b);
    }

    #[test]
    fn a_different_seed_gives_another_identity() {
        let other = SEED.replace("00", "ff");
        let a = parse_node_key(SEED).unwrap().public().to_peer_id();
        let b = parse_node_key(&other).unwrap().public().to_peer_id();
        assert_ne!(a, b);
    }

    #[test]
    fn a_malformed_seed_is_refused() {
        for bad in [
            "",
            "00",
            &SEED[..63],
            &format!("{SEED}00"),
            &SEED.replace("0f", "zz"),
        ] {
            assert!(parse_node_key(bad).is_err(), "{bad}");
        }
        assert!(parse_node_key(&"é".repeat(32)).is_err());
        assert!(
            parse_node_key(&SEED.replace("0f", "+f")).is_err(),
            "a sign is not hex"
        );
    }

    #[test]
    fn a_missing_file_is_refused() {
        let error = load_node_key(Path::new("/no/such/node.key")).unwrap_err();
        assert!(error.contains("node key file"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_file_others_can_read_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("mbongo-node-key-{}", std::process::id()));
        std::fs::write(&path, SEED).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_node_key(&path).unwrap_err().contains("readable by others"));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(load_node_key(&path).is_ok());
        std::fs::remove_file(&path).unwrap();
    }
}
