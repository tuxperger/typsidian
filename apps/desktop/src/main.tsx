import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { confirm } from "@tauri-apps/plugin-dialog";
import "@typsidian/ui/styles/app.css";
import { setConfirmAction } from "@typsidian/ui/confirm.js";
import { DesktopWorkspace } from "./DesktopWorkspace.js";

// Точка входа окна. Всё, что она делает, — монтирует общую рабочую область;
// шрифты и стили те же, что на web (`ui/src/styles`).
// Подтверждение удаления — нативным окном плагина: `window.confirm` в оболочке
// не работает (подробности — `ui/src/confirm.ts`).
setConfirmAction((message) => confirm(message, { kind: "warning" }));

const root = document.getElementById("root");
if (!root) throw new Error("the window document has no #root node");

createRoot(root).render(
  <StrictMode>
    <DesktopWorkspace />
  </StrictMode>,
);
