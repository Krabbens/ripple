import { defineConfig } from "astro/config";
import ripple from "@ripple/plugin/astro";
export default defineConfig({
  integrations: [ripple({ sourceFile: "src/pages/index.astro" })],
  server: { host: "127.0.0.1", port: 4321 },
});
