//! Which network a compute signer signs for (RFC 0006 §2, §7).
//!
//! A transaction signature covers `"mbongo-tx:v2" || chain_id || fields`,
//! so every signer needs the `chain_id` of its network. RFC 0006 §7 makes
//! it the signer's own configuration: an endpoint never chooses the network
//! a user signs for. The binaries take it the way the node does, from
//! exactly one of `--dev` (the built-in dev genesis) and `--genesis <FILE>`,
//! with no default.

use std::io::Read;
use std::path::{Path, PathBuf};

use mbongo_core::genesis::MAX_GENESIS_FILE_BYTES;
use mbongo_core::{ChainId, Genesis};

/// `--dev` or `--genesis <FILE>`, exactly one: the network a binary signs
/// for.
#[derive(clap::Args, Debug, Clone)]
#[group(required = true, multiple = false)]
pub struct NetworkArgs {
    /// Sign for the built-in dev genesis: public test keys only
    #[arg(long)]
    pub dev: bool,

    /// Sign for the network defined by this genesis file (mbongo-genesis:v1)
    #[arg(long, value_name = "FILE")]
    pub genesis: Option<PathBuf>,
}

impl NetworkArgs {
    /// The `chain_id` of the selected network.
    ///
    /// # Errors
    ///
    /// If the genesis file cannot be read or is not a valid
    /// `mbongo-genesis:v1` file.
    pub fn chain_id(&self) -> Result<ChainId, String> {
        match (&self.genesis, self.dev) {
            (Some(path), false) => genesis_file_chain_id(path),
            (None, true) => Ok(Genesis::dev().chain_id),
            // clap enforces the group; this keeps the function total.
            _ => Err("exactly one of --dev and --genesis is required".to_string()),
        }
    }
}

/// The `chain_id` of a genesis file, refused unless it is a valid
/// `mbongo-genesis:v1` file.
///
/// # Errors
///
/// If the file cannot be read or is not valid.
pub fn genesis_file_chain_id(path: &Path) -> Result<ChainId, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("cannot read genesis file {}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    // One byte over the bound, so that the parser refuses an oversized file.
    file.take(MAX_GENESIS_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read genesis file {}: {e}", path.display()))?;
    Genesis::from_file_bytes(&bytes)
        .map(|g| g.chain_id)
        .map_err(|e| format!("invalid genesis file {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        network: NetworkArgs,
    }

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("bin").chain(args.iter().copied()))
    }

    #[test]
    fn dev_is_the_dev_chain() {
        let cli = parse(&["--dev"]).unwrap();
        assert_eq!(cli.network.chain_id().unwrap(), Genesis::dev().chain_id);
    }

    #[test]
    fn a_network_is_required_and_exclusive() {
        let err = parse(&[]).err().unwrap();
        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
        let err = parse(&["--dev", "--genesis", "x.json"]).err().unwrap();
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn a_genesis_file_gives_its_chain_id() {
        let dir = std::env::temp_dir().join(format!("mbongo-network-args-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("g.json");
        std::fs::write(&good, mbongo_core::genesis::DEV_GENESIS_FILE).unwrap();
        let cli = parse(&["--genesis", good.to_str().unwrap()]).unwrap();
        assert_eq!(cli.network.chain_id().unwrap(), Genesis::dev().chain_id);
        let bad = dir.join("bad.json");
        std::fs::write(&bad, b"{}").unwrap();
        let err = genesis_file_chain_id(&bad).err().unwrap();
        assert!(err.contains("invalid genesis file"), "{err}");
        let err = genesis_file_chain_id(&dir.join("missing.json")).err().unwrap();
        assert!(err.contains("cannot read genesis file"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
