import type { Translation } from "../define.js";
import { ruN } from "../define.js";
import type { spellcheck as en } from "../en/spellcheck.js";

// Проверка орфографии (фича 030): меню редактора и раздел настроек vault.
export const spellcheck: Translation<typeof en> = {
  "spellcheck.menu.fix": (p) => `Исправить «${p.word}»`,
  "spellcheck.menu.none": "Нет вариантов",
  "spellcheck.menu.addWord": "Добавить в словарь",
  "spellcheck.addFailed": "Не удалось добавить слово в словарь vault.",
  "spellcheck.settings.section": "Орфография",
  "spellcheck.settings.enabled": "Проверять орфографию",
  "spellcheck.settings.enabledHint": "Опечатки в тексте заметок подчёркиваются; разметка, формулы и код не проверяются",
  "spellcheck.settings.languages": "Языки",
  "spellcheck.settings.languagesHint": "Слово сверяется со словарём своего алфавита",
  "spellcheck.settings.lang.ru": "Русский",
  "spellcheck.settings.lang.en": "English",
  "spellcheck.settings.dictionary": (p) =>
    `Словарь vault · ${ruN(p.count, "# слово", "# слова", "# слов")}`,
  "spellcheck.settings.dictionaryEmpty": "Словарь vault · пусто",
  "spellcheck.settings.empty": "Слова появятся здесь после «Добавить в словарь» в меню подчёркнутого слова.",
  "spellcheck.settings.file": ".typsidian/dictionary.txt",
  "spellcheck.settings.newWord": "Новое слово",
  "spellcheck.settings.add": "Добавить",
  "spellcheck.settings.remove": "убрать",
  "spellcheck.settings.removeLabel": (p) => `Убрать «${p.word}» из словаря`,
  "spellcheck.error.InvalidWord": "Это не слово: только буквы, дефис и апостроф, до 64 символов.",
  "spellcheck.error.ReadOnly": "Vault только для чтения — словарь не изменён.",
  "spellcheck.error.Io": "Не удалось записать словарь vault.",
};
