import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@typsidian/ui/styles/app.css";
import { ShowWindowRoot } from "@typsidian/ui/slides/ShowWindowRoot.js";
import { createTauriShowViewer } from "@typsidian/ui/slides/show-channel-tauri.js";

// Точка входа окна показа слайдов (019 T019, T024). Без рабочей области и без
// ядра: сессия и роль приходят в адресе, который задаёт хост
// (`show.html?session=<id>&role=<role>`), а слайды — по каналу из хоста.
const params = new URLSearchParams(window.location.search);
const session = params.get("session");
if (!session) throw new Error("the show window url has no session parameter");
// Роль по умолчанию — окно показа; докладчик запрашивается явно.
const role = params.get("role") === "presenter" ? "presenter" : "show";

const root = document.getElementById("root");
if (!root) throw new Error("the window document has no #root node");

createRoot(root).render(
  <StrictMode>
    <ShowWindowRoot viewer={createTauriShowViewer(session)} role={role} />
  </StrictMode>,
);
