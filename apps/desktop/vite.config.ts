import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

/**
 * Сборка оболочки рабочего стола (001 T112).
 *
 * Vite, а не Next.js: у desktop нет ни сервера, ни маршрутизации — окно
 * загружает один статический документ, а всё остальное приходит из
 * `@typsidian/ui`. Next здесь был бы сервером, который никто не слушает.
 */
export default defineConfig({
  plugins: [react()],
  // Порт зафиксирован: его же ждёт `devUrl` в tauri.conf.json, и молчаливый
  // переход на свободный порт означал бы пустое окно.
  server: { port: 1420, strictPort: true },
  build: { outDir: "dist", emptyOutDir: true },
});
