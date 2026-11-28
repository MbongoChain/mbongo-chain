//! Which chain a data directory belongs to (RFC 0006 §1; storage schema 4,
//! S1-8).
//!
//! The chain identity lives in the database: `chain_id` and the genesis hash
//! (the hash of block 0), written once, in the genesis batch, with block 0 and
//! the allocations ([`crate::backend::NodeBackend::ensure_genesis`]). Opening a
//! directory is decided by [`RocksDbStorage::open_chain`]: a new directory is
//! created, an interrupted creation of one resumes, this chain reopens, and
//! everything else, earlier schemas and other chains included, is refused
//! before anything in it is written. The rules and their reasons are in
//! `mbongo-storage`'s `rocksdb` module.
//!
//! # The S1-4 marker
//!
//! Before schema 4 the node kept a copy of the genesis file in an
//! `MBONGO_GENESIS` file next to the database. It is no longer written or
//! read. A directory that holds one was created by an earlier version (storage
//! schema 3), so it is refused before the database is opened, with a message
//! that says so; the database's own stamp would refuse it anyway. The file's
//! content is never parsed or trusted.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use mbongo_core::genesis::MAX_GENESIS_FILE_BYTES;
use mbongo_core::Genesis;
pub(crate) use mbongo_storage::Opened;
use mbongo_storage::{ChainIdentity, RocksDbStorage};

use crate::backend::chain_identity;
#[cfg(test)]
use crate::backend::{compute_block_hash, genesis_block};

/// The S1-4 marker file. Its presence refuses a directory; it is never read.
pub(crate) const LEGACY_MARKER: &str = "MBONGO_GENESIS";

/// The genesis the operator selected.
pub(crate) struct SelectedGenesis {
    /// Where it came from, for the start banner.
    pub source: String,
    /// The parsed file.
    pub genesis: Genesis,
}

impl SelectedGenesis {
    /// The built-in dev genesis (`--dev`).
    pub(crate) fn dev() -> Self {
        Self {
            source: "dev (built in)".to_string(),
            genesis: Genesis::dev(),
        }
    }

    /// A genesis file (`--genesis <file>`), refused unless it is a valid
    /// `mbongo-genesis:v1` file.
    pub(crate) fn from_file(path: &Path) -> Result<Self, String> {
        let file = File::open(path)
            .map_err(|e| format!("cannot read genesis file {}: {e}", path.display()))?;
        let mut bytes = Vec::new();
        // One byte over the bound, so that an oversized file is refused by
        // the parser as TooLarge instead of being read whole.
        file.take(MAX_GENESIS_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("cannot read genesis file {}: {e}", path.display()))?;
        let genesis = Genesis::from_file_bytes(&bytes)
            .map_err(|e| format!("invalid genesis file {}: {e}", path.display()))?;
        Ok(Self {
            source: path.display().to_string(),
            genesis,
        })
    }

    /// The identity a directory of this genesis holds: its `chain_id`, and
    /// the hash of the block 0 it derives.
    pub(crate) fn identity(&self) -> ChainIdentity {
        chain_identity(&self.genesis)
    }
}

/// Why a data directory was refused. In every case nothing in it changed.
#[derive(Debug)]
pub(crate) enum DirError {
    /// It holds the S1-4 marker: an earlier version created it.
    LegacyMarker,
    /// Refused by the storage layer, or a storage failure.
    Storage(String),
}

impl std::fmt::Display for DirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LegacyMarker => write!(
                f,
                "the data directory holds an {LEGACY_MARKER} file: an earlier version created \
                 it (storage schema 3), and this node refuses it rather than migrate it. It was \
                 not opened or modified. Start from an empty directory"
            ),
            Self::Storage(e) => write!(f, "storage error: {e}"),
        }
    }
}

impl std::error::Error for DirError {}

/// Opens the data directory of the selected genesis. On success the caller
/// writes the genesis (`NodeBackend::ensure_genesis`), which only checks it
/// for [`Opened::Reopened`].
pub(crate) fn open(
    dir: &Path,
    selected: &SelectedGenesis,
) -> Result<(RocksDbStorage, Opened), DirError> {
    // Presence only: the file is never opened.
    if std::fs::symlink_metadata(dir.join(LEGACY_MARKER)).is_ok() {
        return Err(DirError::LegacyMarker);
    }
    RocksDbStorage::open_chain(dir, &selected.identity())
        .map_err(|e| DirError::Storage(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NodeBackend;
    use mbongo_core::genesis::DEV_GENESIS_FILE;
    use mbongo_storage::Storage;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A fresh path under the temp dir, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mbongo-genesis-dir-{name}-{}-{n}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Every file under `dir` with its bytes, except RocksDB's info logs:
    /// what "nothing changed" means here.
    fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_file() && !name.starts_with("LOG") {
                out.insert(name, fs::read(entry.path()).unwrap());
            }
        }
        out
    }

    /// The dev genesis file with another timestamp: another chain.
    fn genesis_with_timestamp(timestamp: u64) -> SelectedGenesis {
        let text = std::str::from_utf8(DEV_GENESIS_FILE)
            .unwrap()
            .replace("\"timestamp\":0}", &format!("\"timestamp\":{timestamp}}}"));
        let bytes = text.into_bytes();
        let genesis = Genesis::from_file_bytes(&bytes).unwrap();
        SelectedGenesis {
            source: format!("timestamp {timestamp}"),
            genesis,
        }
    }

    /// A full start: open, then the genesis batch.
    fn start(dir: &Path, selected: &SelectedGenesis) -> Result<Opened, DirError> {
        let (storage, opened) = open(dir, selected)?;
        let backend = NodeBackend::new(storage, false);
        backend
            .ensure_genesis(&selected.genesis)
            .map_err(|e| DirError::Storage(e.to_string()))?;
        Ok(opened)
    }

    /// Height, block-0 hash and stored identity of a started directory.
    fn chain_state(dir: &Path) -> (u64, mbongo_core::Hash, Option<ChainIdentity>) {
        let storage = RocksDbStorage::open(dir).unwrap();
        let block = storage.get_block_by_height(0).unwrap().unwrap();
        (
            storage.get_latest_height().unwrap(),
            compute_block_hash(&block),
            storage.chain_identity().unwrap(),
        )
    }

    /// Raw access to a database, to build the states a test needs.
    fn raw(dir: &Path, f: &dyn Fn(&rocksdb::DB)) {
        let names = rocksdb::DB::list_cf(&rocksdb::Options::default(), dir).unwrap();
        let db = rocksdb::DB::open_cf(&rocksdb::Options::default(), dir, &names).unwrap();
        f(&db);
    }

    #[test]
    fn the_identity_is_the_chain_id_and_the_genesis_block_hash() {
        let selected = SelectedGenesis::dev();
        let id = selected.identity();
        assert_eq!(id.chain_id, Genesis::dev().chain_id);
        assert_eq!(
            id.genesis_hash,
            compute_block_hash(&genesis_block(&Genesis::dev()))
        );
        assert_ne!(genesis_with_timestamp(1).identity(), id);
    }

    #[test]
    fn a_new_directory_is_created_with_its_identity_and_reopened() {
        let dir = TempDir::new("new");
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Created
        );
        let (height, block0, identity) = chain_state(dir.path());
        assert_eq!(height, 0);
        assert_eq!(identity, Some(SelectedGenesis::dev().identity()));
        assert_eq!(block0, identity.unwrap().genesis_hash);
        assert!(
            !dir.path().join(LEGACY_MARKER).exists(),
            "no marker is written"
        );

        // Restarts reopen; the identity stays the same.
        for _ in 0..3 {
            assert_eq!(
                start(dir.path(), &SelectedGenesis::dev()).unwrap(),
                Opened::Reopened
            );
        }
        assert_eq!(chain_state(dir.path()).2, identity);
    }

    #[test]
    fn an_existing_empty_directory_is_new() {
        let dir = TempDir::new("empty");
        fs::create_dir_all(dir.path()).unwrap();
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Created
        );
    }

    #[test]
    fn another_genesis_is_refused_and_the_directory_is_unchanged() {
        let dir = TempDir::new("other");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        let before = snapshot(dir.path());
        let other = genesis_with_timestamp(1);
        for _ in 0..2 {
            match start(dir.path(), &other) {
                Err(DirError::Storage(e)) => {
                    assert!(
                        e.contains(&format!("belongs to chain {}", Genesis::dev().chain_id)),
                        "{e}"
                    );
                }
                r => panic!("expected a refusal, got {r:?}"),
            }
        }
        assert_eq!(snapshot(dir.path()), before);
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Reopened
        );
    }

    #[test]
    fn a_directory_with_the_s1_4_marker_is_refused_before_it_is_opened() {
        // The marker alone, and the marker beside a schema-3 database.
        let dir = TempDir::new("marker");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join(LEGACY_MARKER), b"not even a valid marker").unwrap();
        let before = snapshot(dir.path());
        match start(dir.path(), &SelectedGenesis::dev()) {
            Err(e @ DirError::LegacyMarker) => {
                assert!(e.to_string().contains("storage schema 3"), "{e}");
            }
            r => panic!("expected the marker refusal, got {r:?}"),
        }
        assert_eq!(snapshot(dir.path()), before);
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no database was created"
        );

        let dir = TempDir::new("marker-db");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        fs::write(dir.path().join(LEGACY_MARKER), b"x").unwrap();
        let before = snapshot(dir.path());
        assert!(matches!(
            start(dir.path(), &SelectedGenesis::dev()),
            Err(DirError::LegacyMarker)
        ));
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn a_schema_3_directory_without_a_marker_is_refused_unchanged() {
        // What a v0.4 node, or this node before S1-8, leaves: stamp 3 and a
        // chain, no identity.
        let dir = TempDir::new("schema3");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        raw(dir.path(), &|db| {
            let meta = db.cf_handle("meta").unwrap();
            db.put_cf(&meta, b"schema_version", 3u32.to_be_bytes()).unwrap();
            db.delete_cf(&meta, b"chain_id").unwrap();
            db.delete_cf(&meta, b"genesis_hash").unwrap();
        });
        let before = snapshot(dir.path());
        match start(dir.path(), &SelectedGenesis::dev()) {
            Err(DirError::Storage(e)) => {
                assert!(
                    e.contains("storage schema 3") && e.contains("not migrated"),
                    "{e}"
                );
            }
            r => panic!("expected a refusal, got {r:?}"),
        }
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn foreign_files_are_refused_unchanged() {
        let dir = TempDir::new("foreign");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("MBONGO_GENESIS.tmp-1-2-3"), b"left by S1-4").unwrap();
        let before = snapshot(dir.path());
        match start(dir.path(), &SelectedGenesis::dev()) {
            Err(DirError::Storage(e)) => assert!(e.contains("no database"), "{e}"),
            r => panic!("expected a refusal, got {r:?}"),
        }
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn an_interrupted_creation_resumes_for_any_genesis_and_only_once() {
        // Opened, then stopped before the genesis batch: stamp 4, nothing else.
        let dir = TempDir::new("resume");
        let (storage, opened) = open(dir.path(), &SelectedGenesis::dev()).unwrap();
        assert_eq!(opened, Opened::Created);
        drop(storage);
        // Nothing records a chain yet, so the next start decides it.
        let other = genesis_with_timestamp(1);
        assert_eq!(start(dir.path(), &other).unwrap(), Opened::Resumed);
        assert_eq!(chain_state(dir.path()).2, Some(other.identity()));
        assert!(start(dir.path(), &SelectedGenesis::dev()).is_err());
        assert_eq!(start(dir.path(), &other).unwrap(), Opened::Reopened);
    }

    #[test]
    fn racing_starts_with_different_genesis_initialize_exactly_one_chain() {
        const STARTS: u64 = 6;
        const ROUNDS: usize = 25;
        for round in 0..ROUNDS {
            let dir = TempDir::new("race");
            let candidates: Vec<SelectedGenesis> =
                (1..=STARTS).map(genesis_with_timestamp).collect();
            let barrier = std::sync::Barrier::new(candidates.len());
            let results: Vec<Result<Opened, DirError>> = std::thread::scope(|s| {
                let handles: Vec<_> = candidates
                    .iter()
                    .map(|g| {
                        let (barrier, path) = (&barrier, dir.path());
                        s.spawn(move || {
                            barrier.wait();
                            start(path, g)
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            // At most one start opens the directory; if none did (each lost
            // to another's lock), the directory may be new or an interrupted
            // creation, never a chain.
            let winners: Vec<usize> = (0..results.len()).filter(|i| results[*i].is_ok()).collect();
            assert!(winners.len() <= 1, "round {round}: {results:?}");
            let Some(&w) = winners.first() else {
                if dir.path().exists() {
                    let storage = RocksDbStorage::open(dir.path()).unwrap();
                    assert_eq!(storage.chain_identity().unwrap(), None, "round {round}");
                }
                continue;
            };
            assert_eq!(
                chain_state(dir.path()).2,
                Some(candidates[w].identity()),
                "round {round}"
            );
            for (i, g) in candidates.iter().enumerate() {
                if i != w {
                    assert!(start(dir.path(), g).is_err(), "round {round}, start {i}");
                }
            }
            assert_eq!(start(dir.path(), &candidates[w]).unwrap(), Opened::Reopened);
        }
    }

    #[test]
    fn racing_starts_with_the_same_genesis_end_on_one_chain() {
        const STARTS: usize = 6;
        const ROUNDS: usize = 25;
        for round in 0..ROUNDS {
            let dir = TempDir::new("race-same");
            let barrier = std::sync::Barrier::new(STARTS);
            let results: Vec<Result<Opened, DirError>> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..STARTS)
                    .map(|_| {
                        let (barrier, path) = (&barrier, dir.path());
                        s.spawn(move || {
                            barrier.wait();
                            start(path, &SelectedGenesis::dev())
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            let created = results.iter().filter(|r| matches!(r, Ok(Opened::Created))).count();
            assert!(created <= 1, "round {round}: {results:?}");
            // Whatever lost, the directory now starts as this chain.
            assert!(
                start(dir.path(), &SelectedGenesis::dev()).is_ok(),
                "round {round}: {results:?}"
            );
            let (height, block0, identity) = chain_state(dir.path());
            assert_eq!(height, 0);
            assert_eq!(identity, Some(SelectedGenesis::dev().identity()));
            assert_eq!(block0, identity.unwrap().genesis_hash);
        }
    }

    #[test]
    fn a_second_open_is_refused_while_the_directory_is_in_use() {
        let dir = TempDir::new("lock");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        let (held, _) = open(dir.path(), &SelectedGenesis::dev()).unwrap();
        assert!(open(dir.path(), &SelectedGenesis::dev()).is_err());
        drop(held);
        assert_eq!(
            open(dir.path(), &SelectedGenesis::dev()).unwrap().1,
            Opened::Reopened
        );
    }

    #[test]
    fn a_directory_whose_block_0_is_not_its_identity_is_refused_and_left_as_it_is() {
        // Block 0 rewritten in the format before S1-7: the stored identity no
        // longer names the indexed block 0. Refused, not misread or rewritten.
        use parity_scale_codec::Encode;
        #[derive(Encode)]
        struct HeaderBeforeS17 {
            parent_hash: mbongo_core::Hash,
            state_root: mbongo_core::Hash,
            transactions_root: mbongo_core::Hash,
            timestamp: u64,
            height: u64,
        }
        #[derive(Encode)]
        struct BlockBeforeS17 {
            header: HeaderBeforeS17,
            body: mbongo_core::BlockBody,
        }
        let dir = TempDir::new("before-s1-7");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        let current = genesis_block(&Genesis::dev());
        let legacy = BlockBeforeS17 {
            header: HeaderBeforeS17 {
                parent_hash: current.header.parent_hash,
                state_root: current.header.state_root,
                transactions_root: current.header.transactions_root,
                timestamp: current.header.timestamp,
                height: 0,
            },
            body: mbongo_core::BlockBody::default(),
        }
        .encode();
        let legacy_hash = *blake3::hash(&legacy).as_bytes();
        let current_hash = compute_block_hash(&current).0;
        let index_key = 0u64.to_be_bytes();
        raw(dir.path(), &|db| {
            let blocks = db.cf_handle("blocks").unwrap();
            let index = db.cf_handle("height_index").unwrap();
            assert_eq!(db.get_cf(&index, index_key).unwrap().unwrap(), current_hash);
            db.delete_cf(&blocks, current_hash).unwrap();
            db.put_cf(&blocks, legacy_hash, &legacy).unwrap();
            db.put_cf(&index, index_key, legacy_hash).unwrap();
        });
        let before = snapshot(dir.path());
        for _ in 0..2 {
            match start(dir.path(), &SelectedGenesis::dev()) {
                Err(DirError::Storage(e)) => {
                    assert!(e.contains("not this database's block 0"), "{e}")
                }
                r => panic!("expected a refusal, got {r:?}"),
            }
        }
        assert_eq!(snapshot(dir.path()), before);
        raw(dir.path(), &|db| {
            let blocks = db.cf_handle("blocks").unwrap();
            let index = db.cf_handle("height_index").unwrap();
            assert_eq!(db.get_cf(&index, index_key).unwrap().unwrap(), legacy_hash);
            assert_eq!(db.get_cf(&blocks, legacy_hash).unwrap().unwrap(), legacy);
            assert!(db.get_cf(&blocks, current_hash).unwrap().is_none());
        });
    }

    #[test]
    fn a_genesis_file_is_read_and_validated() {
        let dir = TempDir::new("file");
        fs::create_dir_all(dir.path()).unwrap();
        let good = dir.path().join("g.json");
        fs::write(&good, DEV_GENESIS_FILE).unwrap();
        let selected = SelectedGenesis::from_file(&good).unwrap();
        assert_eq!(selected.genesis.chain_id, Genesis::dev().chain_id);
        assert_eq!(selected.identity(), SelectedGenesis::dev().identity());

        let bad = dir.path().join("bad.json");
        fs::write(&bad, [DEV_GENESIS_FILE, b"\n"].concat()).unwrap();
        let err = SelectedGenesis::from_file(&bad).err().unwrap();
        assert!(err.contains("invalid genesis file"), "{err}");

        let missing = dir.path().join("missing.json");
        let err = SelectedGenesis::from_file(&missing).err().unwrap();
        assert!(err.contains("cannot read genesis file"), "{err}");

        let large = dir.path().join("large.json");
        fs::write(&large, vec![b' '; MAX_GENESIS_FILE_BYTES + 10]).unwrap();
        let err = SelectedGenesis::from_file(&large).err().unwrap();
        assert!(err.contains("invalid genesis file"), "{err}");
    }
}
