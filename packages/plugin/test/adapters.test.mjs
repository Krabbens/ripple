import { test } from "node:test";
import assert from "node:assert/strict";
import astro from "../dist/astro.js";
import vite from "../dist/vite.js";
import { Ripple } from "../dist/next.js";
import { createElement } from "react";
import { renderToString } from "react-dom/server";
test("Astro only injects Ripple during development", () => {
  const injected = [];
  const integration = astro({ sourceFile: "src/pages/index.astro" });
  for (const command of ["build", "preview", "sync", "dev"]) {
    integration.hooks["astro:config:setup"]({
      command,
      injectScript: (...args) => injected.push(args),
    });
  }
  assert.equal(injected.length, 1);
  assert.equal(injected[0][0], "page");
  assert.match(injected[0][1], /astro:before-swap/);
});
test("Vite is development-only and React/Next never render panel during SSR", () => {
  const plugin = vite();
  assert.equal(plugin.apply, "serve");
  assert.equal(plugin.transformIndexHtml.order, "pre");
  assert.equal(renderToString(createElement(Ripple)), "");
});
