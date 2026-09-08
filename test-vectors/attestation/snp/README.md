# SEV-SNP attestation fixtures

Real, hardware-generated AMD SEV-SNP evidence captured on AMD EPYC (Milan)
hardware by third parties and published under Apache-2.0. They contain no
secrets: an attestation report is a signed public statement, and a VCEK
certificate is public key material issued by the AMD Key Distribution
Service. They are used by the `hardware_attestation` test group to prove
that the `amd-sev-snp-verifier` adapter parses real reports, verifies the
real VCEK → ASK → ARK chain against the pinned AMD root, rejects tampering,
and — importantly — **refuses these very reports as unbound**: their
`REPORT_DATA` was chosen by the capturing party, never by a Mbongo
challenge, so no fixture here can satisfy a K challenge binding. Only a
live SEV-SNP guest answering a fresh challenge can (see
`docs/architecture/compute-hardware-attestation-adapter.md` §16).

| File | Bytes | SHA-256 | Provenance |
|---|---|---|---|
| `milan-go-sev-guest/attestation-report.bin` | 1184 | `377e6241d3b373ab1df80c0f96978594e7e21f4797dd6ea95e2957e1c1e26060` | `google/go-sev-guest` `verify/testdata/attestation.bin`, commit `609f29a2` (2022-09-23), Apache-2.0 |
| `milan-go-sev-guest/vcek.der` | 1360 | `0d057f9b6e29a69eda9c0154b259567d291c1c08d73a11e9d31ace07c435b6d8` | `google/go-sev-guest` `verify/testdata/vcek.testcer`, same commit — the VCEK of the chip that signed the report above |
| `vcek-milan-other-chip-virtee.der` | 1289 | `3bbfb6ee259f75a95d13168cfdf2e034181bb93c7c016825731cbe8ea16c95e1` | `virtee/sev` `tests/certs_data/vcek_milan.der`, Apache-2.0 — a VCEK of a **different** Milan chip; chains to the same AMD root but does not sign the report above |

Facts about the report, as parsed by the official `sev` crate (8.0.0): version 2,
VMPL 0, guest policy `0xb0000` (**debug allowed**, migration not allowed,
SMT allowed), current and reported TCB bootloader 2 / TEE 0 / SNP 5 /
microcode 68, `REPORT_DATA` = `0102030405` followed by zeros, measurement
`b07af962…872b01`. Because debug is allowed, a production `SnpPolicy`
rejects this report with `DebugEnabled`; the tests that need it to pass a
stage use a policy that explicitly allows debug and say so.

Certificate validity: the VCEK has a fixed X.509 validity window. The
fixture tests disable wall-clock certificate validity enforcement
(`enforce_certificate_validity = false`) because the suite runs on a manual
clock; a production policy enables it, and a test asserts that enabling it
with a clock outside the window is refused (`ExpiredCollateral`).

The AMD ARK and ASK roots are not stored here: they are the ones pinned in
the `sev` crate (`sev::certs::snp::builtin::milan`), and the K trust policy
references them by the BLAKE3 digest of their PEM.

Nothing in this directory is a protocol vector. The protocol vectors are
`compute-task/`, `receipt/`, `rpc/` and `transaction/`.
