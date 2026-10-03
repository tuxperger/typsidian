import type { Translation } from "../define.js";
import type { palette as en } from "../en/palette.js";

export const palette: Translation<typeof en> = {
  "palette.label": "Палитра команд",
  "palette.inputLabel": "Введите команду или название заметки",
  "palette.placeholder": "Введите команду или название заметки…",
  "palette.close": "Закрыть палитру",
  "palette.groupNotes": "Заметки",
  "palette.groupCommands": "Команды",
  "palette.nothingFound": "ничего не найдено",
  "palette.hintNavigate": "навигация",
  "palette.hintOpen": "открыть",
  "palette.hintClose": "esc закрыть",
  "palette.changedNow": "изменено сейчас",
};
