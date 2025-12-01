# Mbongo Chain Crates

This directory contains all the Rust crates that make up the Mbongo Chain blockchain.

## Crate Structure

### Core Libraries

- **mbongo-core**: Core blockchain primitives (types, crypto, storage)
- **mbongo-consensus**: PoX consensus engine and AIDA regulator
- **mbongo-verification**: Multi-layer compute verification system
- **mbongo-compute**: GPU compute execution runtime

### Infrastructure

- **mbongo-network**: P2P networking layer (libp2p)
- **mbongo-runtime**: WebAssembly smart contract VM
- **mbongo-api**: REST and WebSocket APIs

### Applications

- **mbongo-wallet**: Wallet and key management
- **mbongo-node**: Full node binary (main entry point)

## Development Status

⚠️ **All crates are currently skeleton structures** for future development.

Each crate contains:
- `Cargo.toml` with basic dependencies
- `src/lib.rs` or `src/main.rs` with placeholder documentation
- Basic test structure

## Building

```bash
# Build all crates
cargo build --workspace

# Build specific crate
cargo build -p mbongo-core

# Run tests
cargo test --workspace

# Check formatting
cargo fmt --all -- --check

# Run clippy
cargo clippy --all -- -D warnings
```

## Adding Dependencies

All workspace dependencies are managed in the root `Cargo.toml` under `[workspace.dependencies]`.

To add a new dependency:

1. Add it to root `Cargo.toml`:
   ```toml
   [workspace.dependencies]
   my-dep = "1.0"
   ```

2. Reference it in the crate's `Cargo.toml`:
   ```toml
   [dependencies]
   my-dep = { workspace = true }
   ```

## Crate Dependency Graph

```
mbongo-node
├── mbongo-core
├── mbongo-consensus
│   └── mbongo-core
├── mbongo-verification
│   └── mbongo-core
├── mbongo-compute
│   └── mbongo-core
├── mbongo-network
│   └── mbongo-core
├── mbongo-runtime
│   └── mbongo-core
└── mbongo-api
    ├── mbongo-core
    └── mbongo-consensus

mbongo-wallet
└── mbongo-core
```

## Implementation Guidelines

See [CONTRIBUTING.md](../CONTRIBUTING.md) for:
- Code style guidelines
- Testing requirements
- Documentation standards
- Pull request process

## Next Steps for Contributors

1. **Read the docs**: Start with `docs/` folder, especially:
   - `pox_formula.md` - Consensus mechanics
   - `verification_strategy.md` - Verification approach
   - `architecture_overview.md` - System design

2. **Pick a crate**: Choose based on your expertise:
   - Rust blockchain: `mbongo-core`, `mbongo-consensus`
   - Networking: `mbongo-network`
   - GPU/Compute: `mbongo-compute`
   - APIs: `mbongo-api`

3. **Start implementing**: Begin with types and core functionality

4. **Write tests**: Add unit and integration tests as you go

5. **Document**: Keep Rustdoc comments up-to-date

## Questions?

- GitHub Discussions: https://github.com/mbongo-chain/mbongo-chain/discussions
- Discord: https://discord.gg/mbongo-chain (coming soon)
