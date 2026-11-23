//! The key a producer signs its blocks with (RFC 0006 §3).
//!
//! Outside `--dev`, `--producer` needs `--producer-key-file`: a file holding
//! the authority's 32-byte Ed25519 seed as 64 hex characters, as the client
//! key files do. Under `--dev` the public dev authority key is taken instead,
//! and a key file is refused. Either way the key must be the authority of the
//! selected genesis, or the node refuses to start, before it opens anything.

use std::path::Path;

use mbongo_core::{Address, Genesis};

/// How the node was asked to produce, from its flags.
pub(crate) struct ProducerFlags<'a> {
    /// `--producer`.
    pub(crate) producer: bool,
    /// `--dev`.
    pub(crate) dev: bool,
    /// `--producer-key-file`.
    pub(crate) key_file: Option<&'a Path>,
}

/// The seed this node signs blocks with, or `None` for a node that does not
/// produce.
///
/// # Errors
///
/// A description, if the flags combine in a way RFC 0006 §3 refuses, if the
/// key file cannot be read or is malformed, or if the key is not the
/// authority of `genesis`.
pub(crate) fn producer_seed(
    flags: &ProducerFlags<'_>,
    genesis: &Genesis,
) -> Result<Option<[u8; 32]>, String> {
    if flags.key_file.is_some() && flags.dev {
        return Err(
            "--producer-key-file cannot be used with --dev, which signs with the public dev \
             authority key"
                .to_string(),
        );
    }
    if flags.key_file.is_some() && !flags.producer {
        return Err("--producer-key-file is only used with --producer".to_string());
    }
    if !flags.producer {
        return Ok(None);
    }
    let seed = if flags.dev {
        mbongo_core::genesis::dev_authority_seed()
    } else {
        let path = flags.key_file.ok_or_else(|| {
            "--producer needs --producer-key-file, holding the authority's key, outside --dev"
                .to_string()
        })?;
        load_producer_key(path)?
    };
    let key = Address(ed25519_dalek::SigningKey::from_bytes(&seed).verifying_key().to_bytes());
    if key != genesis.authority() {
        return Err(format!(
            "the producer key {key} is not the authority {} of chain {}",
            genesis.authority(),
            genesis.chain_id
        ));
    }
    Ok(Some(seed))
}

/// Reads a producer key file.
///
/// # Errors
///
/// A description if the file cannot be read, does not hold exactly 64 hex
/// characters, or on Unix is readable by anyone but its owner.
pub(crate) fn load_producer_key(path: &Path) -> Result<[u8; 32], String> {
    let shown = path.display();
    check_private(path)?;
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("producer key file {shown}: {e}"))?;
    parse_seed(text.trim()).map_err(|e| format!("producer key file {shown}: {e}"))
}

/// A 32-byte seed from exactly 64 hex characters.
fn parse_seed(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("expected 64 hex characters".to_string());
    }
    let mut seed = [0u8; 32];
    hex::decode_to_slice(text, &mut seed).map_err(|_| "expected 64 hex characters".to_string())?;
    Ok(seed)
}

#[cfg(unix)]
fn check_private(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let shown = path.display();
    let mode = std::fs::metadata(path)
        .map_err(|e| format!("producer key file {shown}: {e}"))?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        return Err(format!(
            "producer key file {shown}: readable by others, expected mode 600"
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

    /// A TEST ONLY seed, public.
    const SEED: [u8; 32] = [0x10; 32];

    /// A genesis whose authority is the key of `SEED`.
    fn genesis_of(seed: &[u8; 32]) -> Genesis {
        let authority = ed25519_dalek::SigningKey::from_bytes(seed).verifying_key().to_bytes();
        let file = format!(
            "{{\"allocations\":[],\"authorities\":[\"{}\"],\"name\":\"producer-key-test\",\
             \"schema\":\"mbongo-genesis:v1\",\"timestamp\":0}}",
            hex::encode(authority)
        );
        Genesis::from_file_bytes(file.as_bytes()).unwrap()
    }

    fn key_file(name: &str, contents: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("mbongo-producer-key-{name}-{}", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        path
    }

    fn flags(producer: bool, dev: bool, key_file: Option<&Path>) -> ProducerFlags<'_> {
        ProducerFlags {
            producer,
            dev,
            key_file,
        }
    }

    #[test]
    fn dev_takes_the_dev_authority_key() {
        let seed = producer_seed(&flags(true, true, None), &Genesis::dev()).unwrap();
        assert_eq!(seed, Some(mbongo_core::genesis::dev_authority_seed()));
    }

    #[test]
    fn a_node_that_does_not_produce_holds_no_key() {
        assert_eq!(
            producer_seed(&flags(false, true, None), &Genesis::dev()),
            Ok(None)
        );
        assert_eq!(
            producer_seed(&flags(false, false, None), &genesis_of(&SEED)),
            Ok(None)
        );
    }

    #[test]
    fn a_private_genesis_takes_the_key_file_of_its_authority() {
        let path = key_file("match", &format!("{}\n", hex::encode(SEED)));
        let seed = producer_seed(&flags(true, false, Some(&path)), &genesis_of(&SEED));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(seed, Ok(Some(SEED)));
    }

    #[test]
    fn a_private_genesis_without_a_key_file_is_refused() {
        let error = producer_seed(&flags(true, false, None), &genesis_of(&SEED)).unwrap_err();
        assert!(
            error.contains("--producer needs --producer-key-file"),
            "{error}"
        );
    }

    #[test]
    fn a_key_that_is_not_the_authority_is_refused() {
        let path = key_file("other", &hex::encode([0x11u8; 32]));
        let error =
            producer_seed(&flags(true, false, Some(&path)), &genesis_of(&SEED)).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert!(error.contains("is not the authority"), "{error}");
    }

    #[test]
    fn the_dev_authority_key_file_does_not_produce_another_network() {
        // A private genesis cannot be produced with the public dev key: its
        // authority is another key.
        let path = key_file(
            "dev",
            &hex::encode(mbongo_core::genesis::dev_authority_seed()),
        );
        let error =
            producer_seed(&flags(true, false, Some(&path)), &genesis_of(&SEED)).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert!(error.contains("is not the authority"), "{error}");
    }

    #[test]
    fn dev_with_a_key_file_is_refused() {
        let path = key_file("dev-and-file", &hex::encode(SEED));
        let refused = [
            producer_seed(&flags(true, true, Some(&path)), &Genesis::dev()),
            producer_seed(&flags(false, true, Some(&path)), &Genesis::dev()),
        ];
        std::fs::remove_file(&path).unwrap();
        for result in refused {
            let error = result.unwrap_err();
            assert!(error.contains("cannot be used with --dev"), "{error}");
        }
    }

    #[test]
    fn a_key_file_without_producer_is_refused() {
        let path = key_file("no-producer", &hex::encode(SEED));
        let error =
            producer_seed(&flags(false, false, Some(&path)), &genesis_of(&SEED)).unwrap_err();
        std::fs::remove_file(&path).unwrap();
        assert!(error.contains("only used with --producer"), "{error}");
    }

    #[test]
    fn a_malformed_or_missing_key_file_is_refused() {
        let good = hex::encode(SEED);
        for (name, bad) in [
            ("empty", String::new()),
            ("short", good[..63].to_string()),
            ("long", format!("{good}00")),
            ("not-hex", good.replace("10", "zz")),
            ("sign", good.replacen("10", "+0", 1)),
            ("prefix", format!("0x{}", &good[2..])),
        ] {
            let path = key_file(name, &bad);
            let error =
                producer_seed(&flags(true, false, Some(&path)), &genesis_of(&SEED)).unwrap_err();
            std::fs::remove_file(&path).unwrap();
            assert!(
                error.contains("expected 64 hex characters"),
                "{name}: {error}"
            );
        }
        let error = producer_seed(
            &flags(true, false, Some(Path::new("/no/such/producer.key"))),
            &genesis_of(&SEED),
        )
        .unwrap_err();
        assert!(error.contains("producer key file"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_file_others_can_read_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let path = key_file("mode", &hex::encode(SEED));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_producer_key(&path).unwrap_err().contains("readable by others"));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(load_producer_key(&path), Ok(SEED));
        std::fs::remove_file(&path).unwrap();
    }
}
