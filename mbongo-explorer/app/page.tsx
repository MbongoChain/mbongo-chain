import Link from "next/link";
import { getBlockHeight, getBlockByHeight } from "@/lib/rpc";
import type { Block } from "@/types/block";

interface BlockEntry {
  height: number;
  stateRoot: string;
  timestamp: number;
}

async function fetchRecentBlocks(): Promise<{
  height: number;
  blocks: BlockEntry[];
} | null> {
  try {
    const height = await getBlockHeight();
    const count = Math.min(10, height + 1);
    const fetches: Promise<Block | null>[] = [];
    for (let i = 0; i < count; i++) {
      fetches.push(
        getBlockByHeight(height - i).catch(() => null),
      );
    }
    const results = await Promise.all(fetches);
    const blocks: BlockEntry[] = [];
    for (const block of results) {
      if (block) {
        blocks.push({
          height: block.header.height,
          stateRoot: block.header.state_root,
          timestamp: block.header.timestamp,
        });
      }
    }
    return { height, blocks };
  } catch {
    return null;
  }
}

function truncateHash(hash: string): string {
  if (hash.length <= 20) return hash;
  return `${hash.slice(0, 10)}...${hash.slice(-8)}`;
}

function formatTimestamp(ts: number): string {
  if (ts === 0) return "genesis";
  return new Date(ts * 1000).toISOString().replace("T", " ").replace(".000Z", " UTC");
}

export default async function HomePage() {
  const data = await fetchRecentBlocks();

  if (!data) {
    return (
      <div className="error-card">
        <h2>Node unavailable</h2>
        <p>
          Could not connect to the Mbongo Chain RPC endpoint.
          Ensure the node is running and NEXT_PUBLIC_RPC_URL is configured.
        </p>
      </div>
    );
  }

  return (
    <>
      <div className="stat-card">
        <div className="stat-label">Current Block Height</div>
        <div className="stat-value">{data.height}</div>
      </div>

      <h2 className="page-title">Recent Blocks</h2>
      <div className="block-list">
        {data.blocks.map((block) => (
          <div key={block.height} className="block-row">
            <Link href={`/block/${block.height}`} className="block-link">
              <span className="block-height">#{block.height}</span>
              <span className="block-hash" title={block.stateRoot}>
                {truncateHash(block.stateRoot)}
              </span>
            </Link>
          </div>
        ))}
        {data.blocks.length === 0 && (
          <div className="block-row">
            <span className="block-hash">No blocks found</span>
          </div>
        )}
      </div>
    </>
  );
}
