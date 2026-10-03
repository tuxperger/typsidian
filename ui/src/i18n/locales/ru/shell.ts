import type { Translation } from "../define.js";
import type { shell as en } from "../en/shell.js";

export const shell: Translation<typeof en> = {
  "shell.pickFolderTitle": "Выберите папку с заметками",
  "picker.title": "Папка с заметками",
  "picker.up": "↑ Наверх",
  "picker.noSubfolders":
    "Вложенных папок здесь нет — эту можно выбрать целиком или создать в ней новую.",
  "picker.createFolder": "Создать папку",
  "picker.chooseThis": "Выбрать эту папку",
  "picker.newName": "Имя новой папки",
  "picker.create": "Создать",
  "picker.readFailed": (p) => `Не удалось прочитать папку: ${p.detail}`,
  "picker.accessDenied":
    "Нет доступа к памяти устройства. Разрешите Typsidian «Доступ ко всем файлам» в настройках Android и откройте обзор заново.",
  "picker.notFound": "Этой папки больше нет: её переименовали или удалили.",
  "picker.notADirectory": "Это файл, а не папка.",
  "picker.alreadyExists": "Папка с таким именем здесь уже есть.",
  "picker.invalidName": "Имя папки не может быть пустым и не может содержать «/».",
  "picker.io": (p) => `Система отказала: ${p.detail}`,
};
