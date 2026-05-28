import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

// Vite picks the port for the Tauri webview dev server. We pin 1421 (not the
// chatter-gui 1420) so both desktop apps can be cargo-tauri-dev'd in parallel
// without colliding on the dev port.
export default defineConfig(async () => ({
  plugins: [react()],

  // Prevent vite from obscuring Rust errors
  clearScreen: false,

  server: {
    port: 1421,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1422,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
