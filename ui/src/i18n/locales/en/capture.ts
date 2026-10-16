import { enN, type Entries } from "../define.js";

// Быстрый захват и ежедневные заметки (`capture.*`) — фича 022.
export const capture = {
  // ── Лист захвата (D1–D3) ───────────────────────────────────────────
  "capture.sheet.label": "Quick capture",
  "capture.sheet.title": "To Typsidian",
  "capture.sheet.close": "Close",
  "capture.manual.label": "What to write down",
  "capture.manual.placeholder": "A thought, a link, a note…",
  "capture.image.alt": (p: { name: string }) => `Preview of ${p.name}`,
  "capture.image.caption": (p: { name: string; size: string }) => `${p.name} · ${p.size}`,
  "capture.dest.label": "Where to put it",
  "capture.dest.inbox": "Inbox",
  "capture.dest.daily": "Today",
  "capture.dest.recent": "Recent",
  "capture.dest.recentChoose": "Recent…",
  "capture.dest.recentNone": "No open notes",
  "capture.note.label": "Comment",
  "capture.note.placeholder": "Comment…",
  "capture.save": "Save",
  "capture.saving": "Saving…",
  "capture.hint.append": (p: { path: string; time: string }) =>
    `Will be appended to the end of “${p.path}” · ${p.time}`,
  "capture.hint.image": (p: { path: string }) =>
    `The picture goes to attachments, the embed goes to “${p.path}”`,
  "capture.hint.unknown": "The destination note will be shown once it is known",
  "capture.queue.more": (p: { count: number }) => `${enN(p.count, "# more capture", "# more captures")} in the queue`,

  // ── Отказ (D3) ─────────────────────────────────────────────────────
  "capture.failed.title": "The capture was not written",
  "capture.failed.noVault.title": "No access to the vault folder",
  "capture.failed.noVault.body":
    "The capture is saved and waits: open Typsidian and choose the folder again.",
  "capture.failed.pending": (p: { count: number }) =>
    `Waiting to be written: ${enN(p.count, "# capture", "# captures")}`,
  "capture.failed.open": "Open Typsidian",
  "capture.failed.retry": "Try again",
  "capture.failed.discard": "Discard the capture",

  // ── Команды ────────────────────────────────────────────────────────
  "capture.command.inbox": "To inbox",
  "capture.command.today": "Today",
  "capture.command.previousDay": "Previous day",
  "capture.command.nextDay": "Next day",

  // ── «Сегодня» и ‹ › (D4) ───────────────────────────────────────────
  "capture.daily.today": "Today",
  "capture.daily.todayHint": "Open today’s daily note",
  "capture.daily.previous": "Previous daily note",
  "capture.daily.next": "Next daily note",
  "capture.daily.nav": "Daily notes",
  "capture.daily.noPrevious": "No earlier daily note",
  "capture.daily.noNext": "No later daily note",
  "capture.daily.templateMissing":
    "The daily note template was not found: the note was created with a heading only",
  "capture.daily.failed": (p: { reason: string }) => `Could not open today’s note: ${p.reason}`,
  "capture.daily.error.InvalidDate": "the date is invalid",
  "capture.daily.error.ReadOnly": "the vault is read-only",
  "capture.daily.error.NoVault": "no vault is open",
  "capture.daily.error.Io": "the disk refused the write",

  // ── Настройки vault (D6) ───────────────────────────────────────────
  "capture.settings.title": "Capture and journal",
  "capture.settings.openVault": "Open a vault to configure capture.",
  "capture.settings.inbox": "Inbox note",
  "capture.settings.inboxHint": "Created on the first capture if it does not exist",
  "capture.settings.dailyFolder": "Daily notes folder",
  "capture.settings.dailyTemplate": "Daily note template",
  "capture.settings.dailyTemplateHint":
    "One of the templates; without it a new note gets only a heading with its file name",
  "capture.settings.noTemplate": "No template",
  "capture.settings.dailyName": "Date-based name",
  "capture.settings.dailyNameToday": (p: { path: string }) => `Today: ${p.path}`,
  "capture.settings.name.YYYY-MM-DD": "YYYY-MM-DD",
  "capture.settings.name.YYYY/MM/YYYY-MM-DD": "YYYY/MM/YYYY-MM-DD",
  "capture.settings.name.DD.MM.YYYY": "DD.MM.YYYY",
  "capture.settings.entryStyle": "Capture entry style",
  "capture.settings.style.list": "List item",
  "capture.settings.style.paragraph": "Paragraph",
  "capture.settings.style.time": "With time",
  "capture.settings.sample": "Sample entry",
  "capture.settings.sampleTitle": "How the PostgreSQL planner works",
  "capture.settings.sampleUrl": "https://habr.com/en/articles/851234",
  "capture.settings.sampleNote": "read before the talk",
  "capture.settings.footer": "Stored with the vault: the same on all devices",
  "capture.settings.invalid": (p: { field: string; problem: string }) =>
    `“${p.field}” cannot be used: ${p.problem}`,
} as const satisfies Entries;
