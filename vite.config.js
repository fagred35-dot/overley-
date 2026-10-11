import { defineConfig } from "vite";
import { resolve } from "node:path";

const frontendRoot = resolve(process.cwd(), "src");

export default defineConfig({
  root: frontendRoot,
  clearScreen: false,
  server: {
    host: "0.0.0.0",
    port: 1420,
    strictPort: true,
    // The Tauri webview uses localhost; this also permits Arena's preview host.
    allowedHosts: [".e2b.app"],
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: resolve(frontendRoot, "index.html"),
        overlay: resolve(frontendRoot, "overlay.html"),
      },
    },
  },
});
