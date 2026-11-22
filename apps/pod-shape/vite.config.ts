import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri runs the dev server on a fixed port and loads dist/ in the release build.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1430, strictPort: true },
  build: { target: "safari16", outDir: "dist" },
});
