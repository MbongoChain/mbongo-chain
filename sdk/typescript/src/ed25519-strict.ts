/**
 * Strict Ed25519 verification, as RFC 0006 §3 defines it and §2 applies to
 * transaction and receipt signatures.
 *
 * A signature is valid only if all of these hold:
 * - `s` is below the group order;
 * - the public key and `R` are canonical encodings;
 * - neither the public key nor `R` is of small order;
 * - the verification equation holds **without** the cofactor.
 *
 * The library's `verify` is not used for the verdict. By default it applies
 * ZIP-215 rules: it accepts non-canonical encodings and checks the cofactored
 * equation. In its RFC 8032 mode it still checks the cofactored equation, and
 * does not refuse a small-order `R`. Using it would let this client accept
 * signatures the node refuses. The point arithmetic is still the library's.
 *
 * Agreement with the node is checked against
 * `test-vectors/signature/ed25519-strict-v1.json`, which Rust also reads.
 *
 * Internal: not re-exported from the package entry point.
 */

import { ed25519 } from "@noble/curves/ed25519.js";
import { sha512 } from "@noble/hashes/sha2.js";

/** The group order `L = 2^252 + 27742317777372353535851937790883648493`. */
const GROUP_ORDER = 2n ** 252n + 27742317777372353535851937790883648493n;

const Point = ed25519.Point;

function littleEndian(bytes: Uint8Array): bigint {
  let n = 0n;
  for (let i = bytes.length - 1; i >= 0; i--) n = (n << 8n) | BigInt(bytes[i]);
  return n;
}

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a[i] ^ b[i];
  return diff === 0;
}

/**
 * Decodes a point that must be canonical: the RFC 8032 decoding (`y < p`, no
 * sign bit on `x = 0`), confirmed by re-encoding to the same bytes.
 */
function canonicalPoint(bytes: Uint8Array): ReturnType<typeof Point.fromBytes> | null {
  try {
    const p = Point.fromBytes(bytes, false);
    return sameBytes(p.toBytes(), bytes) ? p : null;
  } catch {
    return null;
  }
}

/**
 * Verifies `signature` over `message` by `publicKey` under the strict
 * definition. Returns `false` for any malformed input; never throws.
 */
export function verifyEd25519Strict(
  signature: Uint8Array,
  message: Uint8Array,
  publicKey: Uint8Array,
): boolean {
  if (!(signature instanceof Uint8Array) || signature.length !== 64) return false;
  if (!(publicKey instanceof Uint8Array) || publicKey.length !== 32) return false;
  if (!(message instanceof Uint8Array)) return false;
  const rBytes = signature.subarray(0, 32);
  const s = littleEndian(signature.subarray(32));
  if (s >= GROUP_ORDER) return false;
  const A = canonicalPoint(publicKey);
  const R = canonicalPoint(rBytes);
  if (A === null || R === null) return false;
  if (A.isSmallOrder() || R.isSmallOrder()) return false;
  try {
    const k = littleEndian(sha512(new Uint8Array([...rBytes, ...publicKey, ...message]))) % GROUP_ORDER;
    // [s]B = R + [k]A, without the cofactor.
    const left = s === 0n ? Point.ZERO : Point.BASE.multiplyUnsafe(s);
    const right = k === 0n ? R : R.add(A.multiplyUnsafe(k));
    return left.equals(right);
  } catch {
    return false;
  }
}
