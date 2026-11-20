// Solo desarrollo: la interfaz con una simulación de Tauri, para capturas en el navegador.
import { defineConfig } from "vite";
import preact from "@preact/preset-vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
  plugins: [preact()],
  resolve: {
    alias: {
      "@tauri-apps/api/core": fileURLToPath(new URL("./dev/tauri-core.ts", import.meta.url)),
      "@tauri-apps/api/event": fileURLToPath(new URL("./dev/tauri-event.ts", import.meta.url)),
    },
  },
  server: { port: 5199, strictPort: true },
});
