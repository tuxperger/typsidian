import type { Translation } from "../define.js";
import type { embeds as en } from "../en/embeds.js";

// Встраивание заметок (фича 031): команда, группы дополнения, отметки редактора.
export const embeds: Translation<typeof en> = {
  "embeds.command": "Встроить заметку",
  "embeds.insertFailed": "Не удалось добавить библиотеку встраивания в vault.",
  "embeds.group.sections": "Разделы",
  "embeds.kind.section": "раздел",
  "embeds.mark.missingPart": (p) => `В заметке «${p.target}» нет раздела или метки «${p.part}»`,
  "embeds.mark.missingPartDetail": "В предпросмотре на месте встраивания — плашка.",
};
