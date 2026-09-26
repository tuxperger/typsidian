import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

/**
 * Сборка мобильной оболочки.
 *
 * Тот же Vite и та же схема, что у рабочего стола (`apps/desktop`): окно
 * загружает один статический документ, всё остальное приходит из
 * `@typsidian/ui`.
 */
export default defineConfig({
  plugins: [react()],
  // Порт зафиксирован: его же ждёт `devUrl` в tauri.conf.json, и молчаливый
  // переход на свободный порт означал бы пустое окно. 1421, а не 1420, —
  // чтобы `tauri dev` мобильной оболочки и рабочего стола можно было держать
  // запущенными одновременно.
  server: {
    port: 1421,
    strictPort: true,
    // Телефон ходит на этот сервер по сети, а не через localhost: `tauri
    // android dev` пробрасывает адрес хоста в WebView устройства. Слушать
    // только петлю значило бы отдать устройству пустое окно.
    host: "0.0.0.0",
  },
  build: { outDir: "dist", emptyOutDir: true },
});
