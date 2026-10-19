// Выбор файлов изображений (фича 016, research.md §2).
//
// Один путь на три оболочки: скрытый `<input type="file">` открывает системный
// диалог и в браузере, и в WebKitGTK, и в Android WebView — там же камеру и
// галерею, без плагинов. Что из выбранного изображение, решает ядро по
// содержимому: фильтр `accept` — только подсказка диалогу.

export type PickSource = "files" | "gallery" | "camera";

/** Отмена диалога — пустой массив: текст заметки не меняется (FR-022). */
export function pickFiles(source: PickSource): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    if (source === "camera") {
      input.accept = "image/*";
      input.setAttribute("capture", "environment");
    } else {
      input.multiple = true;
      if (source === "gallery") input.accept = "image/*";
    }
    input.style.display = "none";
    let settled = false;
    const finish = (files: File[]): void => {
      if (settled) return;
      settled = true;
      input.remove();
      resolve(files);
    };
    input.addEventListener("change", () => finish(Array.from(input.files ?? [])));
    // `cancel` есть не везде; без него выбор просто не придёт, и обещание
    // остаётся висеть до следующего выбора — вставка при этом не начинается.
    input.addEventListener("cancel", () => finish([]));
    document.body.append(input);
    input.click();
  });
}
