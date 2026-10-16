import { DomSurface } from "@ripple/core";
import { mountHarness } from "@ripple/harness";
import type { HarnessOptions } from "@ripple/harness";

export interface RippleOptions extends HarnessOptions {
  sourceFile?: string;
  mount?: HTMLElement;
}
let active: { destroy(): void; leases: number } | undefined;

/** Explicit browser entry. Framework adapters call it only in development. */
export function mountRipple(options: RippleOptions = {}): { destroy(): void } {
  if (typeof document === "undefined") return { destroy() {} };
  if (!active) {
    const harness = mountHarness(
      new DomSurface(options.mount ?? document.body, options.sourceFile),
      "",
      {
        ...options,
        endpoint: options.endpoint ?? "http://127.0.0.1:4310",
        reserveSpace: options.reserveSpace ?? false,
      },
    );
    active = { destroy: harness.destroy, leases: 0 };
  }
  const instance = active;
  instance.leases++;
  let released = false;
  return {
    destroy() {
      if (released) return;
      released = true;
      if (--instance.leases === 0) {
        instance.destroy();
        if (active === instance) active = undefined;
      }
    },
  };
}
