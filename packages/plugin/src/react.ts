"use client";
import { useEffect } from "react";
import type { RippleOptions } from "./index.js";

/** Null during SSR and in production. Effect cleanup also handles React StrictMode. */
export function Ripple({
  endpoint,
  sourceFile,
  reserveSpace,
  onApplied,
}: RippleOptions = {}): null {
  useEffect(() => {
    if (process.env.NODE_ENV === "production") return;
    let cancelled = false;
    let handle: { destroy(): void } | undefined;
    void import("./index.js").then(({ mountRipple }) => {
      if (!cancelled)
        handle = mountRipple({
          ...(endpoint !== undefined ? { endpoint } : {}),
          ...(sourceFile !== undefined ? { sourceFile } : {}),
          ...(reserveSpace !== undefined ? { reserveSpace } : {}),
          ...(onApplied !== undefined ? { onApplied } : {}),
        });
    });
    return () => {
      cancelled = true;
      handle?.destroy();
    };
  }, [endpoint, sourceFile, reserveSpace, onApplied]);
  return null;
}
