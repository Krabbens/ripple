import type { AstroIntegration } from "astro";
import type { RippleOptions } from "./index.js";
type Options = Pick<RippleOptions, "endpoint" | "sourceFile" | "reserveSpace">;
export default function ripple(options: Options = {}): AstroIntegration {
  return {
    name: "@ripple/plugin",
    hooks: {
      "astro:config:setup": ({ command, injectScript }) => {
        if (command !== "dev") return;
        const config = JSON.stringify(options).replace(/</g, "\\u003c");
        injectScript(
          "page",
          `import { mountRipple } from "@ripple/plugin";
      let panel;
      const mount = () => { panel?.destroy(); panel = mountRipple(${config}); };
      document.addEventListener("astro:before-swap", () => { panel?.destroy(); panel = undefined; });
      document.addEventListener("astro:page-load", mount);
      mount();
      if (import.meta.hot) import.meta.hot.dispose(() => panel?.destroy());`,
        );
      },
    },
  };
}
