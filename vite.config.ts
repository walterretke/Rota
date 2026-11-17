import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Port fixed so `tauri dev` always finds the dev server.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // the Rust core lives in src/main: never watch its build output
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src/main/**"] } },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
  test: { include: ["src/**/*.test.ts", "tests/**/*.test.ts"] },
});
