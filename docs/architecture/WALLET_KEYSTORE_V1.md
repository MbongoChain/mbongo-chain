# Wallet Keystore — `mbongo-keystore:v1`

| | |
|---|---|
| **Status** | **PROPOSED, NON-NORMATIVE.** A design for review (S2 B0). It authorizes no implementation, and becomes binding only when the maintainer approves it. |
| **Kind** | Off-chain file format and local key handling for `mbongo-wallet`. Not consensus, not a protocol surface. |
| **Date** | 2026-11-08 |
| **Changes nothing in** | Ed25519 transaction, receipt or block signatures; RFC 0006 (#211) and its gates; S0; S1; the MVP v0.2 gates; any code |
| **Related** | #206 (S2), #248 (post-quantum research), [`MVP_v0.2_NETWORK_READINESS.md`](../product/MVP_v0.2_NETWORK_READINESS.md) AC2 |

## 1. Scope

A file that holds one secret signing key, encrypted at rest under a passphrase, so that
a test-network wallet can create a key and show its address (AC2).

**In scope:** the file format, key derivation, encryption, the algorithm registry, key
generation, memory handling, file permissions, atomic writes, export and import, and the
tests and vectors an implementation must pass.

**Not in scope:** signing transactions (it waits for the signing domain of S1-5),
transfers, a faucet, hierarchical or hardware wallets, mnemonics, a production-wallet
claim, and any new signature algorithm.

**Protects against:** an attacker who obtains a copy of the file but not the passphrase,
from a stolen disk, backup or synchronized folder. **Does not protect against:** malware
on the host while the key is unlocked, keyloggers, a weak passphrase, or a quantum
computer breaking Ed25519 itself (#248). The symmetric parts, Argon2id and
XChaCha20-Poly1305 with 256-bit keys, are not the weak link there.

## 2. Format

A file is one UTF-8 JSON object, with no byte-order mark, at most **65,536 bytes**.

```json
{
  "schema": "mbongo-keystore:v1",
  "key": {
    "algorithm": "ed25519",
    "encoding": "seed-32",
    "public_key": "3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3b"
  },
  "kdf": {
    "algorithm": "argon2id",
    "version": 19,
    "m_kib": 65536,
    "t": 3,
    "p": 4,
    "salt": "000102030405060708090a0b0c0d0e0f"
  },
  "cipher": {
    "algorithm": "xchacha20poly1305",
    "nonce": "101112131415161718191a1b1c1d1e1f2021222324252627"
  },
  "ciphertext": "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"
}
```

**This example only shows the shape and the field lengths.** Its bytes are fillers: it
is not a valid keystore, and no key decrypts it. Real test vectors are in section 10.

| Field | Type | Exact content |
|---|---|---|
| `schema` | string | `mbongo-keystore:v1` |
| `key.algorithm` | string | An identifier of section 3 |
| `key.encoding` | string | The encoding section 3 pairs with that algorithm |
| `key.public_key` | hex | The public key. For `ed25519`: 32 bytes, 64 hex characters |
| `kdf.algorithm` | string | `argon2id` |
| `kdf.version` | integer | `19` (Argon2 version 0x13) |
| `kdf.m_kib` | integer | Memory in KiB, within section 5 |
| `kdf.t` | integer | Passes, within section 5 |
| `kdf.p` | integer | Lanes, within section 5 |
| `kdf.salt` | hex | 16 bytes, 32 hex characters |
| `cipher.algorithm` | string | `xchacha20poly1305` |
| `cipher.nonce` | hex | 24 bytes, 48 hex characters |
| `ciphertext` | hex | The encrypted secret followed by the 16-byte tag. For `ed25519` / `seed-32`: 48 bytes, 96 hex characters |

### 2.1 Strict parsing

A reader **refuses** the file, before any key derivation, if any of these holds:

- it is larger than 65,536 bytes, is not UTF-8, or starts with a byte-order mark;
- it is not exactly one JSON object;
- any object has a **duplicate key**, an **unknown key**, or lacks a key listed above.
  Duplicates are refused at every level, even when both values agree;
- a value has the wrong JSON type;
- an integer has a sign, a fraction, an exponent or a leading zero, or does not fit in
  `u32`;
- a hex string has uppercase letters, an odd length, a `0x` prefix, or the wrong length
  for its field;
- an identifier is unknown, or reserved but unsupported (section 3);
- a KDF parameter is outside section 5.

Whitespace between JSON tokens is insignificant. A **writer** emits the keys in the order
of the table, two-space indentation, and one final newline, so that files it writes are
byte-for-byte predictable.

## 3. Algorithm registry

Closed: an identifier not listed here is refused. Adding one, or moving a reserved one to
supported, is an amendment of this document with its own review. It needs no new schema
as long as the fields of section 2 suffice.

| Field | Identifier | Status | Encoding and sizes |
|---|---|---|---|
| `key.algorithm` | `ed25519` | **Supported** | `seed-32`: 32-byte seed (RFC 8032); public key 32 bytes |
| `key.algorithm` | `ml-dsa-44`, `ml-dsa-65`, `ml-dsa-87` | Reserved, unsupported | `seed-32` (FIPS 204 key-generation seed); public keys of 1,312, 1,952 and 2,592 bytes |
| `key.algorithm` | `slh-dsa-sha2-128s` | Reserved, unsupported | `seed-3n`, n = 16: 48 bytes (FIPS 205); public key 32 bytes |
| `key.algorithm` | `hybrid-ed25519+ml-dsa-65` | Reserved, unsupported | `seed-32+seed-32`: 64 bytes; public key 32 + 1,952 bytes, concatenated |
| `kdf.algorithm` | `argon2id` | **Supported** | Version 19 only |
| `cipher.algorithm` | `xchacha20poly1305` | **Supported** | 32-byte key, 24-byte nonce, 16-byte tag |

A reader reports a reserved identifier as **unsupported**, which is a different error
from **unknown**. Reserving an identifier implements nothing, adopts no post-quantum
scheme, and changes no protocol signature: see #248. The 65,536-byte limit leaves room for
the largest reserved public key in hex.

## 4. Authenticated metadata

Every field except `ciphertext` is authenticated, so that no algorithm, parameter, salt,
nonce or public key can be altered or downgraded without the decryption failing.

The associated data is computed **from the parsed values**, not from the JSON text, so it
does not depend on whitespace or key order:

```text
field(x) = u32_le(len(x)) || x
AAD = field("mbongo-keystore:v1")
   || field(key.algorithm) || field(key.encoding) || field(public_key bytes)
   || field(kdf.algorithm) || u32_le(version) || u32_le(m_kib) || u32_le(t) || u32_le(p)
   || field(salt bytes)
   || field(cipher.algorithm) || field(nonce bytes)
```

Strings are their UTF-8 bytes; hex fields are their decoded bytes.

## 5. Key derivation

`key = Argon2id(password = P, salt, m = m_kib, t, p, version = 0x13, tag length = 32)`,
per RFC 9106, with no secret value and no associated data inside Argon2.

| Parameter | Accepted range | Written by default |
|---|---|---|
| `m_kib` | 19,456 to 1,048,576 (19 MiB to 1 GiB), and at least 8 × `p` | 65,536 (64 MiB) |
| `t` | 2 to 10 | 3 |
| `p` | 1 to 8 | 4 |
| `salt` | exactly 16 bytes, from the operating system's random source | fresh for each file |

**Why these values.** The default is the second recommended option of RFC 9106 §4, for
memory-constrained use: 64 MiB, 3 passes, 4 lanes, 128-bit salt, 256-bit tag. The lower
bounds are the OWASP Password Storage Cheat Sheet's Argon2id configuration of 19 MiB and
2 passes, and both must hold, so no accepted file is weaker than it. The upper bounds stop a crafted
file from making a reader allocate unbounded memory or spin for minutes: they are
**checked before** the derivation starts. A reader may set a lower local memory ceiling,
and then refuses files above it with a resource error.

**Calibration.** The target is an unlock of about 0.5 to 2 seconds on the reference
machine. It is measured during implementation (B1), not asserted here. A writer may
lower the parameters within the ranges on a small machine; a reader accepts any value
within them.

## 6. Passphrase

- **Normalization:** the passphrase is a Unicode string, normalized to **NFC**, then
  encoded as UTF-8. `P` is those bytes. The same visible passphrase therefore gives the
  same key whether it was typed as composed or decomposed characters.
- **Refused:** an empty passphrase; one containing a C0 or C1 control character,
  U+0000 included; fewer than 12 code points after normalization; more than 1,024 bytes
  after encoding. Nothing is trimmed.
- **Input:** an interactive terminal without echo, asked twice when a key is created; or
  `--passphrase-file <path>`, whose content is the passphrase, with one final `\n` or
  `\r\n` removed, under the same permission rules as a keystore (section 9). **Never** an
  environment variable or a command-line argument, which other processes and shell
  histories can read.
- A passphrase is never logged, echoed, or included in an error.

## 7. Generation, encryption and memory

**Creating a key:**

1. 32 bytes from the operating system's random source (`getrandom`) form the seed.
2. The Ed25519 public key is derived from the seed (RFC 8032).
3. A fresh 16-byte salt and a fresh 24-byte nonce come from the same source.
4. `key` is derived (section 5), and `ciphertext = XChaCha20-Poly1305(key, nonce, seed,
   AAD)`.
5. The file is serialized, then **decrypted again in memory** and checked against the
   public key before it is written.
6. The file is written atomically (section 9).

**Unlocking a key**, in this order: size; strict parsing; registry; parameter ranges; local
resource ceiling; derivation; decryption with the AAD; plaintext length; public key
derived from the seed, compared in constant time with `public_key`.

**Memory:** the seed, the derived key, the passphrase bytes and the normalization buffer
are zeroed after use (`zeroize`). They are never formatted, logged or kept in a type that
prints its content. Locking memory against swap is not required in v1, and the risk of
swap and hibernation files is documented for users.

## 8. Errors

| Error | When | Says |
|---|---|---|
| `format` | Section 2.1 | Which rule failed. No secret is involved |
| `unsupported` | A reserved identifier | The identifier |
| `resource` | Parameters above the local ceiling | The ceiling |
| `unlock` | Decryption fails | **Only** "wrong passphrase or corrupted file". Never which byte, field or step |
| `inconsistent` | Decryption succeeds but the public key does not match | That the file is inconsistent |
| `permissions` | Section 9 | The path and the rule |
| `exists` | The destination exists | The path |

## 9. Storage, permissions and atomic writes

**Location:** `$XDG_DATA_HOME/mbongo/keys` (by default `~/.local/share/mbongo/keys`) on
Unix, and `%APPDATA%\mbongo\keys` on Windows. One key per file, named `<name>.json`,
where `<name>` matches `[a-z0-9-]{1,32}`.

**Unix:**

- the directory is created with mode `0700`, and files with `0600`;
- a reader refuses a keystore that it does not own, or whose mode grants anything to the
  group or to others, as `ssh` does with private keys.

**Windows (candidate policy):**

- on creation, the file gets a protected access list that grants full control to the
  current user only, with no inherited entries;
- a reader **warns** when the file's access list grants read to Everyone, Authenticated
  Users, Users or Guests.

Whether to warn or refuse, and the Windows API dependency, are open decisions
(section 13).

**Atomic writes, never overwriting:**

1. Create `.<name>.json.tmp-<random>` in the same directory, exclusively, with the final
   permissions.
2. Write, then flush to disk.
3. Publish without replacing:
   - Unix: `link` the temporary file to `<name>.json`, which fails if it exists, then
     remove the temporary file, then flush the directory;
   - Windows: `MoveFileEx` without `MOVEFILE_REPLACE_EXISTING`, with
     `MOVEFILE_WRITE_THROUGH`.
4. An existing `<name>.json` is never replaced. There is no force option in v1.

A crash leaves at most a temporary file. Readers ignore it, and `key list` reports it.

## 10. Test vectors and negative tests

**Vectors.** At least three, committed under `test-vectors/wallet/` by the implementation:

- one with the default parameters;
- one at the lower bounds;
- one whose passphrase contains decomposed characters, to prove NFC normalization.

Each fixes the seed, passphrase, salt, nonce and parameters, and lists the expected public
key, AAD, derived key and ciphertext.

They are produced by **two independent implementations**:

- the Rust implementation, built on the RustCrypto `argon2` and `chacha20poly1305` crates;
- a script outside the repository, using libargon2 (reference implementation) and
  libsodium's XChaCha20-Poly1305.

The two must agree byte for byte. A vector produced by only one of them does not count.

**Negative tests, each asserting its own error:**

- a wrong passphrase;
- one flipped bit in the ciphertext;
- each authenticated field altered in turn;
- a duplicate key at every level;
- an unknown key;
- a missing key;
- a reserved identifier and an unknown identifier;
- each KDF bound exceeded by one, refused **before** any derivation (asserted by time and
  memory);
- wrong hex lengths, uppercase hex, a `0x` prefix;
- a truncated file, an empty file, a file of 65,537 bytes, a byte-order mark;
- a matching ciphertext with a different public key (`inconsistent`);
- an existing destination (`exists`);
- a file or directory open to others, on Unix;
- a passphrase that is empty, too short, too long, or contains a control character.

## 11. Backup and recovery

- **Export** copies the file's bytes unchanged, after strict parsing. It decrypts only
  when asked to verify.
- **Import** parses strictly, optionally verifies the unlock, and writes atomically
  without overwriting.
- Recovering a key needs **both** the file and the passphrase. Losing either loses the
  key: there is no recovery service and no plaintext export in v1. Users are told to keep
  at least two offline copies.
- Changing the passphrase writes a new file with a fresh salt and nonce. The old file is
  removed only after the new one has been verified (a later command).

## 12. Dependencies

| Crate | Use | In the workspace today |
|---|---|---|
| `ed25519-dalek` | Key derivation from the seed | Yes |
| `getrandom` | Seed, salt, nonce | Yes |
| `chacha20poly1305` | XChaCha20-Poly1305 | Yes (`mbongo-compute`) |
| `zeroize` | Clearing secrets | Indirectly; becomes direct |
| `serde`, `serde_json` | Parsing into structures that refuse unknown and duplicate fields | Yes |
| `argon2` (RustCrypto) | Argon2id | **No** |
| `unicode-normalization` | NFC | **No** |
| `rpassword` | Passphrase without echo | **No** |
| `windows-sys` or equivalent | Access lists on Windows | Only if the Windows policy is adopted |

Each new crate needs, before implementation: a licence compatible with Apache-2.0, a
minimum Rust version of 1.85 or lower, no open advisory in `cargo-audit`, and active
maintenance.

`serde_json` refuses duplicate fields only when parsing into a derived structure.
Generic maps keep the last value silently. The implementation must parse into structures
that refuse unknown fields, and the duplicate-key tests prove it.

## 13. Decisions open before implementation

1. Argon2id with XChaCha20-Poly1305, as here, or the `age` format.
2. The default and bounds of section 5, after measurement on the reference machine.
3. NFC normalization, against byte-exact passphrases, and the 12-code-point minimum.
4. Windows: refuse or warn on a broad access list, and the dependency it needs.
5. Whether to lock memory against swap.
6. The new dependencies of section 12.
7. The location of the keys directory on each platform.
