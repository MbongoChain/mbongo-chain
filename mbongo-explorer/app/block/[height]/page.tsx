import Link from "next/link";
import { getBlockByHeight } from "@/lib/rpc";
import type { Block } from "@/types/block";
import { RawJson } from "./raw-json";

interface PageProps {
  params: Promise<{ height: string }>;
}

function formatTimestamp(ts: number): string {
  if (ts === 0) return "genesis";
  return new Date(ts * 1000).toISOString().replace("T", " ").replace(".000Z", " UTC");
}

export default async function BlockDetailPage({ params }: PageProps) {
  const { height: heightParam } = await params;
  const height = parseInt(heightParam, 10);

  if (isNaN(height) || height < 0) {
    return (
      <>
        <Link href="/" className="back-link">&larr; Back to blocks</Link>
        <div className="error-card">
          <h2>Invalid block height</h2>
          <p>Block height must be a non-negative integer.</p>
        </div>
      </>
    );
  }

  let block: Block;
  try {
    block = await getBlockByHeight(height);
  } catch {
    return (
      <>
        <Link href="/" className="back-link">&larr; Back to blocks</Link>
        <div className="error-card">
          <h2>Node unavailable</h2>
          <p>
            Could not fetch block at height {height}.
            The node may be offline or the block may not exist.
          </p>
        </div>
      </>
    );
  }

  return (
    <>
      <Link href="/" className="back-link">&larr; Back to blocks</Link>

      <h2 className="page-title">Block #{height}</h2>

      <div className="detail-table">
        <div className="detail-row">
          <span className="detail-label">Height</span>
          <span className="detail-value">{block.header.height}</span>
        </div>
        <div className="detail-row">
          <span className="detail-label">State Root</span>
          <span className="detail-value">{block.header.state_root}</span>
        </div>
        <div className="detail-row">
          <span className="detail-label">Parent Hash</span>
          <span className="detail-value">{block.header.parent_hash}</span>
        </div>
        <div className="detail-row">
          <span className="detail-label">Transactions Root</span>
          <span className="detail-value">{block.header.transactions_root}</span>
        </div>
        <div className="detail-row">
          <span className="detail-label">Timestamp</span>
          <span className="detail-value">{formatTimestamp(block.header.timestamp)}</span>
        </div>
        <div className="detail-row">
          <span className="detail-label">Transaction Count</span>
          <span className="detail-value">{block.body.transactions.length}</span>
        </div>
      </div>

      <RawJson data={JSON.stringify(block, null, 2)} />
    </>
  );
}
