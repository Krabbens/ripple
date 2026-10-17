import { defineConfig } from "vite";
import ripple from "@ripple/plugin/vite";
export default defineConfig({
  root: import.meta.dirname,
  plugins: [ripple({ sourceFile: "src/main.tsx" })],
  server: { host: "127.0.0.1", port: 5173, strictPort: true },
});
