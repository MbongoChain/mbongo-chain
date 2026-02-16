# Code Style

---

## Rust Rules

### Formatting

- **cargo fmt mandatory.** All code must pass `cargo fmt -- --check`. No exceptions.

### Linting

- **clippy clean.** `cargo clippy -- -D warnings` must pass. No allowed lints without justification in code comment.

### Error Handling

- **no unwrap in prod.** Use `?` or explicit `match`/`if let`. `unwrap()` and `expect()` are allowed only in tests or when failure is mathematically impossible.

### Serialization

- **SCALE-only block encoding.** Blocks and transactions use parity-scale-codec for on-chain serialization. No JSON or custom binary for consensus-critical data.

### Documentation

- **public items documented.** All `pub` functions, structs, enums, and traits must have rustdoc comments with summary, args, returns, and example where applicable.

### Determinism

- **deterministic state only.** State transitions must be deterministic. No randomness in execution path. No system-time-dependent logic in state computation.
