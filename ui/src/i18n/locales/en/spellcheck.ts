import type { Entries } from "../define.js";
import { enN } from "../define.js";

// Spellcheck (feature 030): editor menu and the vault settings section.
export const spellcheck = {
  "spellcheck.menu.fix": (p: { word: string }) => `Correct “${p.word}”`,
  "spellcheck.menu.none": "No suggestions",
  "spellcheck.menu.addWord": "Add to dictionary",
  "spellcheck.addFailed": "Couldn’t add the word to the vault dictionary.",
  "spellcheck.settings.section": "Spelling",
  "spellcheck.settings.enabled": "Check spelling",
  "spellcheck.settings.enabledHint": "Typos in note text are underlined; markup, math and code are not checked",
  "spellcheck.settings.languages": "Languages",
  "spellcheck.settings.languagesHint": "Each word is checked against the dictionary of its alphabet",
  "spellcheck.settings.lang.ru": "Русский",
  "spellcheck.settings.lang.en": "English",
  "spellcheck.settings.dictionary": (p: { count: number }) =>
    `Vault dictionary · ${enN(p.count, "# word", "# words")}`,
  "spellcheck.settings.dictionaryEmpty": "Vault dictionary · empty",
  "spellcheck.settings.empty": "Words appear here after “Add to dictionary” in the menu of an underlined word.",
  "spellcheck.settings.file": ".typsidian/dictionary.txt",
  "spellcheck.settings.newWord": "New word",
  "spellcheck.settings.add": "Add",
  "spellcheck.settings.remove": "remove",
  "spellcheck.settings.removeLabel": (p: { word: string }) => `Remove “${p.word}” from the dictionary`,
  "spellcheck.error.InvalidWord": "Not a word: letters, hyphen and apostrophe only, up to 64 characters.",
  "spellcheck.error.ReadOnly": "The vault is read-only — the dictionary was not changed.",
  "spellcheck.error.Io": "Couldn’t write the vault dictionary.",
} as const satisfies Entries;
