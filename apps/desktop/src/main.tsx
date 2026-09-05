import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@typsidian/ui/styles/app.css";
import { DesktopWorkspace } from "./DesktopWorkspace.js";

// Точка входа окна. Всё, что она делает, — монтирует общую рабочую область;
// шрифты и стили те же, что на web (`ui/src/styles`).
const root = document.getElementById("root");
if (!root) throw new Error("в документе окна нет узла #root");

createRoot(root).render(
  <StrictMode>
    <DesktopWorkspace />
  </StrictMode>,
);
