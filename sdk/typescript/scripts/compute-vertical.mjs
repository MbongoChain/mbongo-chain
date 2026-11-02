#!/usr/bin/env node
/**
 * The public SDK's part of the Mbongo Compute v0.4 vertical.
 *
 * Driven by `compute_vertical` (crates/mbongo-compute/src/bin) and usable by
 * hand. Two subcommands, each reading one JSON object on stdin and writing
 * one JSON object on stdout; diagnostics go to stderr. Nothing here prints a
 * secret key, a private input or a private result: keys arrive on stdin,
 * are used, and are never echoed.
 *
 *   node scripts/compute-vertical.mjs submit
 *       stdin: { rpcUrl, submitterSeedHex, executorHex, saltHex,
 *                inputCommitmentHex, executionSpecHex, nonce, timeoutSecs }
 *       Builds the RFC 0005 task, derives task_id, signs the transaction with
 *       the submitter key, submits it through rpc_v0.3, then reads blocks
 *       until the task is committed and decodes it back.
 *
 *   node scripts/compute-vertical.mjs observe
 *       stdin: { rpcUrl, task: { version, submitterHex, executorHex, saltHex,
 *                inputCommitmentHex, executionSpecHex }, executorSeedHex,
 *                outputCommitmentHex, fromHeight, timeoutSecs }
 *       Reads blocks until a receipt for the task is anchored, verifies its
 *       signature and its binding to the task, checks the anchoring
 *       transaction's sender, and compares the anchored bytes with the
 *       receipt `signBoundReceipt` builds for the same task and commitment.
 *
 * Uses the built workspace SDK (`../dist`), never the published package:
 * `@mbongo/sdk` 0.1.0 has no ComputeTask support.
 */

import {
  MbongoClient,
  computeTaskId,
  computeTasksInBlock,
  signComputeTaskTransaction,
  submitComputeTask,
  signBoundReceipt,
  assertReceiptBoundToTask,
  verifyReceiptSignature,
  receiptsInBlock,
  encodeReceipt,
} from "../dist/index.js";

const hex = (bytes) => Buffer.from(bytes).toString("hex");
const unhex = (s) => {
  const t = String(s).replace(/^0x/, "");
  if (t.length % 2 !== 0 || /[^0-9a-fA-F]/.test(t)) throw new Error("bad hex input");
  return Uint8Array.from(Buffer.from(t, "hex"));
};
const equal = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function taskFrom(t) {
  return {
    version: t.version ?? 1,
    submitter: unhex(t.submitterHex),
    executor: unhex(t.executorHex),
    salt: unhex(t.saltHex),
    inputCommitment: unhex(t.inputCommitmentHex),
    executionSpec: unhex(t.executionSpecHex),
  };
}

async function submit(input) {
  const client = new MbongoClient(input.rpcUrl);
  const pong = await client.ping();
  const startHeight = await client.getBlockHeight();
  const secret = unhex(input.submitterSeedHex);
  const task = {
    version: 1,
    submitter: null, // filled below from the key, so the SDK's own check runs
    executor: unhex(input.executorHex),
    salt: unhex(input.saltHex),
    inputCommitment: unhex(input.inputCommitmentHex),
    executionSpec: unhex(input.executionSpecHex),
  };
  // The submitter is the key's public half; the SDK refuses a mismatch.
  const { ed25519 } = await import("@noble/curves/ed25519.js");
  task.submitter = ed25519.getPublicKey(secret);
  const taskId = computeTaskId(task);
  const tx = signComputeTaskTransaction(task, BigInt(input.nonce), secret);
  secret.fill(0);
  const txHash = await submitComputeTask(client, tx);

  const deadline = Date.now() + (input.timeoutSecs ?? 60) * 1000;
  let next = startHeight;
  for (;;) {
    const latest = await client.getBlockHeight();
    while (next <= latest) {
      const block = await client.getBlockByHeight(next);
      const found = computeTasksInBlock(block).find((t) => equal(computeTaskId(t), taskId));
      if (found) {
        return {
          ping: pong,
          startHeight: startHeight.toString(),
          txHash,
          taskId: hex(taskId),
          includedHeight: next.toString(),
          decoded: {
            submitter: hex(found.submitter),
            executor: hex(found.executor),
            salt: hex(found.salt),
            inputCommitment: hex(found.inputCommitment),
            executionSpec: hex(found.executionSpec),
          },
          decodedTaskIdMatches: true,
        };
      }
      next += 1n;
    }
    if (Date.now() > deadline) throw new Error("timed out waiting for the task to be committed");
    await sleep(300);
  }
}

async function observe(input) {
  const client = new MbongoClient(input.rpcUrl);
  const task = taskFrom(input.task);
  const taskId = computeTaskId(task);
  const outputCommitment = unhex(input.outputCommitmentHex);
  const deadline = Date.now() + (input.timeoutSecs ?? 60) * 1000;
  let next = BigInt(input.fromHeight ?? 0);
  for (;;) {
    const latest = await client.getBlockHeight();
    while (next <= latest) {
      const block = await client.getBlockByHeight(next);
      const receipt = receiptsInBlock(block).find((r) => equal(r.taskId, taskId));
      if (receipt) {
        const signatureValid = verifyReceiptSignature(receipt);
        let bound = true;
        let bindingError = null;
        try {
          assertReceiptBoundToTask(receipt, task);
        } catch (e) {
          bound = false;
          bindingError = String(e && e.message ? e.message : e);
        }
        const anchorTx = block.body.transactions.find(
          (t) =>
            t.payload &&
            typeof t.payload === "object" &&
            "AnchorReceipt" in t.payload &&
            equal(unhex(String(t.payload.AnchorReceipt.executor)), receipt.executor),
        );
        const executorSecret = unhex(input.executorSeedHex);
        const rebuilt = signBoundReceipt(task, { outputCommitment }, executorSecret);
        executorSecret.fill(0);
        return {
          anchorHeight: next.toString(),
          anchorTxSender: anchorTx ? String(anchorTx.sender).replace(/^0x/, "") : null,
          receiptExecutor: hex(receipt.executor),
          receiptTaskId: hex(receipt.taskId),
          receiptInputCommitment: hex(receipt.inputCommitment),
          receiptOutputCommitment: hex(receipt.outputCommitment),
          metadataLength: receipt.metadata.length,
          signatureValid,
          bound,
          bindingError,
          identicalToSdkBoundReceipt: equal(encodeReceipt(rebuilt), encodeReceipt(receipt)),
          receiptBytesHex: hex(encodeReceipt(receipt)),
        };
      }
      next += 1n;
    }
    if (Date.now() > deadline) throw new Error("timed out waiting for the receipt to be anchored");
    await sleep(300);
  }
}

const command = process.argv[2];
try {
  const input = await readStdin();
  const out = command === "submit" ? await submit(input) : command === "observe" ? await observe(input) : null;
  if (out === null) {
    process.stderr.write("usage: compute-vertical.mjs submit|observe  (JSON on stdin)\n");
    process.exit(2);
  }
  process.stdout.write(JSON.stringify(out) + "\n");
} catch (e) {
  process.stderr.write(`compute-vertical ${command}: ${e && e.stack ? e.stack : e}\n`);
  process.exit(1);
}
