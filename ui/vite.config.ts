import { defineConfig } from "vite";
import preact from "@preact/preset-vite";

export default defineConfig({
  plugins: [preact()],
  clearScreen: false,
  server: { port: 5174, strictPort: true },
  build: {
    target: "es2022",
    sourcemap: false,
    cssMinify: true,
    // Un solo bundle chico: la app es local, no hay beneficio en dividir.
    rollupOptions: { output: { manualChunks: undefined } },
  },
});
