/**
 * The transaction signing domain of RFC 0006 section 2.
 *
 * A transaction signature covers
 *
 * ```
 * "mbongo-tx:v2" || chain_id[32] || transaction fields
 * ```
 *
 * where the fields are exactly the bytes the transaction encoding holds
 * before its signature. A signature made for one network does not verify on
 * another, and a v0.4 signature, over the bare fields, verifies on none.
 *
 * `chain_id` is the caller's own configuration: the `chain_id` of the genesis
 * of the network it means to sign for (RFC 0006 section 7). An endpoint never
 * chooses it: nothing in this SDK reads it from a node.
 */

/** `"mbongo-tx:v2"` in ASCII, 12 bytes. */
export const TX_SIGNING_DOMAIN: Uint8Array = Uint8Array.from(
  Array.from("mbongo-tx:v2", (c) => c.charCodeAt(0)),
);

/** A `chain_id` is 32 bytes: BLAKE3 of the domain and the genesis file. */
export const CHAIN_ID_BYTES = 32;

/**
 * The bytes a transaction signature covers: the domain, `chainId`, then
 * `fields`. Copies its inputs; never mutates them.
 *
 * @throws {RangeError} `chainId` is not 32 bytes.
 */
export function transactionSigningPayload(
  fields: Uint8Array,
  chainId: Uint8Array,
): Uint8Array {
  if (!(chainId instanceof Uint8Array) || chainId.length !== CHAIN_ID_BYTES) {
    throw new RangeError(`chainId: expected ${CHAIN_ID_BYTES} bytes`);
  }
  const out = new Uint8Array(TX_SIGNING_DOMAIN.length + CHAIN_ID_BYTES + fields.length);
  out.set(TX_SIGNING_DOMAIN, 0);
  out.set(chainId, TX_SIGNING_DOMAIN.length);
  out.set(fields, TX_SIGNING_DOMAIN.length + CHAIN_ID_BYTES);
  return out;
}
