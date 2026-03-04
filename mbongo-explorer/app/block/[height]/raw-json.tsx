"use client";

import { useState } from "react";

export function RawJson({ data }: { data: string }) {
  const [open, setOpen] = useState(false);

  return (
    <>
      <button
        className="raw-json-toggle"
        onClick={() => setOpen((v) => !v)}
        type="button"
      >
        {open ? "Hide" : "Show"} raw JSON
      </button>
      {open && (
        <div className="raw-json-content">
          <pre>{data}</pre>
        </div>
      )}
    </>
  );
}
