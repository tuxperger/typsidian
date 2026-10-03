import { ruN, type Translation } from "../define.js";
import type { common as en } from "../en/common.js";

export const common: Translation<typeof en> = {
  "common.close": "Закрыть",
  "common.cancel": "Отмена",
  "common.save": "Сохранить",
  "common.delete": "Удалить",
  "common.rename": "Переименовать",
  "common.retry": "Повторить",
  "common.loading": "Загрузка…",
  "common.notesCount": (p) => ruN(p.count, "# заметка", "# заметки", "# заметок"),

  "common.clipboard.copyFailed":
    "Скопировать не удалось: среда не дала доступа к буферу обмена. Используйте Ctrl+C.",
  "common.clipboard.pasteFailed":
    "Вставить из меню не удалось: среда не дала прочитать буфер обмена. Используйте Ctrl+V.",

  "common.ok": "Понятно",
};
