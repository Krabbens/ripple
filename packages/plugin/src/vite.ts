import type { Plugin } from "vite";
import type { RippleOptions } from "./index.js";
type Options = Pick<RippleOptions, "endpoint" | "sourceFile" | "reserveSpace">;
export default function ripple(options: Options = {}): Plugin {
  return {
    name: "ripple",
    apply: "serve",
    transformIndexHtml: {
      order: "pre",
      handler: () => [
        {
          tag: "script",
          attrs: { type: "module" },
          injectTo: "body",
          children: `import { mountRipple } from "@ripple/plugin";
      const panel = mountRipple(${JSON.stringify(options).replace(/</g, "\\u003c")});
      if (import.meta.hot) import.meta.hot.dispose(() => panel.destroy());`,
        },
      ],
    },
  };
}
