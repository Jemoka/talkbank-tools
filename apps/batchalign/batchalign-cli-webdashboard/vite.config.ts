import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [
    react({ babel: { plugins: [["babel-plugin-react-compiler"]] } }),
    tailwindcss(),
  ],
  // Absolute asset paths keep deep links stable (/dashboard/jobs/:id) and
  // work with both Rust server static hosting and Tauri's app protocol.
  base: "/",
  build: {
    // Keep React dashboard artifacts local; retired Python server paths are shim-only.
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    proxy: {
      "/jobs": "http://localhost:8000",
      "/health": "http://localhost:8000",
      "/fleet": "http://localhost:8000",
      "/ws": { target: "ws://localhost:8000", ws: true },
    },
  },
});
