import { enN, type Entries } from "../define.js";

// Knowledge graph from lessons (`knowledge.*`) — feature 054, mockup A–F.
export const knowledge = {
  "knowledge.command": "Knowledge graph",
  "knowledge.title": "Knowledge graph",
  "knowledge.open": "Knowledge graph",
  "knowledge.loading": "Loading the knowledge graph…",
  "knowledge.loadFailed": "The knowledge graph could not be loaded.",
  "knowledge.back": "Back",

  // ── Bar: view, search, counters (A, B) ─────────────────────────────
  "knowledge.mode.label": "Knowledge graph view",
  "knowledge.mode.map": "Map",
  "knowledge.mode.chains": "Chains",
  "knowledge.search": "Find a statement…",
  "knowledge.statements": (p: { count: number }) => enN(p.count, "# statement", "# statements"),
  "knowledge.chainsCount": (p: { count: number }) => enN(p.count, "# chain", "# chains"),
  "knowledge.sourcesCount": (p: { count: number }) => enN(p.count, "# source", "# sources"),
  "knowledge.count": (p: { statements: string; chains: string; sources: string }) =>
    `${p.statements} · ${p.chains} · ${p.sources}`,
  "knowledge.canvasLabel": (p: { statements: string; chains: string }) => `Knowledge map: ${p.statements}, ${p.chains}`,

  // ── Empty graph (F), search without results, damaged files ─────────
  "knowledge.empty": "The graph fills up from lessons: the teacher explains — statements and the links between them appear here.",
  "knowledge.emptySearch": "Nothing found. Try other words.",
  "knowledge.damaged": "The knowledge graph files cannot be read in full. What can be read is shown; nothing is written until they are fixed.",

  // ── Map legend ─────────────────────────────────────────────────────
  "knowledge.legend.title": "Legend",
  "knowledge.legend.statement": "statement",
  "knowledge.legend.fork": "fork of a chain",
  "knowledge.legend.selected": "selected",
  "knowledge.zoomIn": "Zoom in",
  "knowledge.zoomOut": "Zoom out",
  "knowledge.zoomFit": "Fit to screen",

  // ── Statement column (A) ───────────────────────────────────────────
  "knowledge.panel.label": "Statement",
  "knowledge.panel.empty": "Choose a statement on the map or in the table to see its wordings, chains and sources.",
  "knowledge.panel.loading": "Loading the statement…",
  "knowledge.panel.gone": "This statement is no longer in the graph.",
  "knowledge.panel.close": "Close",
  "knowledge.panel.also": (p: { text: string }) => `Also: “${p.text}”`,
  "knowledge.panel.derived": "Derived from",
  "knowledge.panel.usedIn": "Used in",
  "knowledge.panel.sources": "Sources",
  "knowledge.panel.self": "this statement",
  "knowledge.panel.incomplete": "incomplete",
  "knowledge.panel.noChains": "Not derived from anything yet.",

  "knowledge.source.event": (p: { n: number }) => `reply ${p.n}`,
  "knowledge.source.removed": "source deleted",
  "knowledge.source.open": (p: { name: string }) => `Open ${p.name}`,

  // ── Chains table (B) ───────────────────────────────────────────────
  "knowledge.chains.label": "Reasoning chains",
  "knowledge.chains.premises": "Premises",
  "knowledge.chains.conclusion": "Conclusion",
  "knowledge.chains.source": "Source",
  "knowledge.chains.empty": "No chains yet: the teacher has not derived one thing from another so far.",

  // ── Deleting a statement (E) ───────────────────────────────────────
  "knowledge.remove.start": "Delete statement…",
  "knowledge.remove.confirm": (p: { text: string }) => `Delete the statement “${p.text}”?`,
  "knowledge.remove.incomplete": (p: { count: number }) =>
    enN(p.count, "# chain where it is a premise becomes incomplete.", "# chains where it is a premise become incomplete."),
  "knowledge.remove.hidden": (p: { count: number }) =>
    enN(p.count, "# chain where it is the conclusion is hidden.", "# chains where it is the conclusion are hidden."),
  "knowledge.remove.cancel": "Cancel",
  "knowledge.remove.do": "Delete",

  // ── Extraction job strip (A) and the lesson (C) ────────────────────
  "knowledge.job.label": "Filling the knowledge graph",
  "knowledge.job.running": (p: { name: string }) => `Filling the graph from “${p.name}”`,
  "knowledge.job.batches": (p: { done: number; total: number }) => `part ${p.done} of ${p.total}`,
  "knowledge.job.starting": "preparing",
  "knowledge.job.cancel": "Cancel",
  "knowledge.job.failed": (p: { name: string }) => `Could not fill the graph from “${p.name}”.`,
  "knowledge.job.failedKept": "What was already added stays in the graph.",
  "knowledge.job.dismiss": "Dismiss",

  "knowledge.lesson.summary": (p: { statements: string; chains: string }) => `Added to the graph: ${p.statements} and ${p.chains}`,
  "knowledge.lesson.summaryMerged": (p: { count: number }) => `${enN(p.count, "# statement", "# statements")} already in the graph were merged.`,
  "knowledge.lesson.summaryNone": "Nothing new was added to the graph.",
  "knowledge.lesson.open": "open",
  "knowledge.lesson.fill": "Fill the graph",
  "knowledge.lesson.fillHint": (p: { count: string }) => `Not in the graph yet: about ${p.count} characters of explanations.`,
  "knowledge.lesson.failed": "The graph could not be filled.",

  // ── Settings (D) ───────────────────────────────────────────────────
  "knowledge.settings.title": "Knowledge graph",
  "knowledge.settings.auto": "Fill the graph after lesson steps",
  "knowledge.settings.autoHint": "Only the teacher's explanations go into the graph, not your answers.",
  "knowledge.settings.agentReads": "The agent may read the graph itself",
  "knowledge.settings.agentReadsHint": "Otherwise the graph goes to the model only by your choice.",
  "knowledge.settings.map": "Show as a map",
  "knowledge.settings.mapHint": "Off — only the table of chains.",

  // ── In the lesson feed ─────────────────────────────────────────────
  "knowledge.read.sent": (p: { query: string; count: number }) =>
    `The agent read the knowledge graph for “${p.query}” — ${enN(p.count, "# statement", "# statements")} sent`,
  "knowledge.read.denied": (p: { query: string }) => `The agent asked for the knowledge graph for “${p.query}” — the folder is closed to the agent`,
  "knowledge.material": (p: { count: number }) => `Based on the knowledge graph: ${enN(p.count, "# statement", "# statements")}`,

  // ── Choosing statements in the new lesson / practice dialogs ───────
  "knowledge.pick.label": "Support from the knowledge graph",
  "knowledge.pick.add": "Add…",
  "knowledge.pick.search": "Search the graph",
  "knowledge.pick.none": "The graph is empty: statements appear here after lessons.",
  "knowledge.pick.noMatches": "No statements found.",
  "knowledge.pick.remove": (p: { text: string }) => `Remove “${p.text}”`,
  "knowledge.pick.limit": (p: { max: number }) => `At most ${p.max} statements.`,
  "knowledge.pick.hint": "The text of the chosen statements goes to the model and is visible in the session and in the log.",
  "knowledge.pick.failed": "The graph could not be loaded.",

  // ── Deleting a lesson (E) ──────────────────────────────────────────
  "knowledge.delete.action": "Delete…",
  "knowledge.delete.aria": (p: { goal: string }) => `Delete lesson “${p.goal}”`,
  "knowledge.delete.title": (p: { goal: string }) => `Delete the lesson “${p.goal}”?`,
  "knowledge.delete.keep": "Keep its contribution to the graph",
  "knowledge.delete.drop": (p: { statements: string; chains: string }) => `Delete the contribution too: ${p.statements}, ${p.chains}`,
  "knowledge.delete.dropNone": "Delete the contribution too",
  "knowledge.delete.cancel": "Cancel",
  "knowledge.delete.do": "Delete lesson",
  "knowledge.delete.noGraph": "The lesson file will be deleted. It has no contribution to the knowledge graph.",


  // ── Import of notes into the graph (feature 055, mockup A–D) ───────
  "knowledge.import.menu": "Add to the knowledge graph…",
  "knowledge.import.add": "Add notes…",
  "knowledge.import.estimating": "Counting what will be sent…",
  "knowledge.import.confirm.title": (p: { name: string }) => `Add “${p.name}” to the knowledge graph?`,
  "knowledge.import.confirm.titleMany": (p: { count: number }) =>
    `Add ${enN(p.count, "# selected item", "# selected items")} to the knowledge graph?`,
  "knowledge.import.files": "Files sent to the model",
  "knowledge.import.filesValue": (p: { total: string; kinds: string }) => `${p.total} (${p.kinds})`,
  "knowledge.import.kindNotes": (p: { count: number }) => enN(p.count, "# note", "# notes"),
  "knowledge.import.kindText": (p: { count: number }) => enN(p.count, "# text file", "# text files"),
  "knowledge.import.kindPdf": (p: { count: number }) => enN(p.count, "# PDF", "# PDFs"),
  "knowledge.import.fileCount": (p: { count: number }) => enN(p.count, "# file", "# files"),
  "knowledge.import.volume": "Volume",
  "knowledge.import.volumeK": (p: { value: string }) => `≈ ${p.value}k units`,
  "knowledge.import.volumeUnits": (p: { value: string }) => `≈ ${p.value} units`,
  "knowledge.import.cost": "Estimated cost",
  "knowledge.import.costValue": (p: { value: string }) => `≈ $${p.value}`,
  "knowledge.import.costTiny": "< $0.01",
  "knowledge.import.costUnknown": "unknown",
  "knowledge.import.costWithModel": (p: { cost: string; model: string }) => `${p.cost} · ${p.model}`,
  "knowledge.import.skipped": "Skipped",
  "knowledge.import.skippedValue": (p: { count: number }) => enN(p.count, "# file", "# files"),
  "knowledge.import.skippedShow": "Show",
  "knowledge.import.skippedHide": "Hide",
  "knowledge.import.skippedMore": (p: { count: number }) => `and ${p.count} more`,
  "knowledge.import.covered": (p: { count: number }) =>
    `${enN(p.count, "# part is", "# parts are")} already in the graph and will not be sent again.`,
  "knowledge.import.nothingNew": "Nothing new to send: everything chosen is already in the graph or was skipped.",
  "knowledge.import.hint": "The text of the files goes into the sent log. The import runs in the background and can be cancelled.",
  "knowledge.import.cancel": "Cancel",
  "knowledge.import.start": "Add",
  "knowledge.import.reestimate": "Estimate again",
  "knowledge.import.reason.Denied": "folder is closed to the agent",
  "knowledge.import.reason.Unsupported": "not supported",
  "knowledge.import.reason.NoText": "no text layer",
  "knowledge.import.reason.Missing": "file not found",
  "knowledge.import.reason.TooLarge": "too large",

  "knowledge.import.job.label": "Import into the knowledge graph",
  "knowledge.import.job.title": (p: { done: number; total: number }) => `Import into the knowledge graph · ${p.done} of ${p.total}`,
  "knowledge.import.job.starting": "Import into the knowledge graph · preparing",
  "knowledge.import.job.cancel": "Cancel",

  "knowledge.import.done.title": "Import finished",
  "knowledge.import.done.cancelled": "Import cancelled",
  "knowledge.import.done.stopped": "Import stopped",
  "knowledge.import.done.added": (p: { statements: string }) => `Added to the graph: ${p.statements}.`,
  "knowledge.import.done.addedNone": "Nothing new was added to the graph.",
  "knowledge.import.done.kept": "What was already added stays in the graph.",
  "knowledge.import.done.close": "Close",
  "knowledge.import.done.openGraph": "Open the graph",

  "knowledge.import.file.Done": "done",
  "knowledge.import.file.Added": (p: { count: number }) => `+${p.count} st.`,
  "knowledge.import.file.Running": (p: { done: number; total: number }) => `part ${p.done} of ${p.total}`,
  "knowledge.import.file.Pending": "waiting",
  "knowledge.import.file.NotSent": "not sent",
  "knowledge.import.file.Partial": (p: { count: number }) =>
    `partial: ${enN(p.count, "# part", "# parts")} not processed`,
  "knowledge.import.file.Failed": (p: { why: string }) => `failed: ${p.why}`,
  "knowledge.import.file.FailedPlain": "failed",
  "knowledge.import.file.Skipped": (p: { why: string }) => `skipped: ${p.why}`,
  "knowledge.import.file.retry": "Retry",
  "knowledge.import.file.retryAria": (p: { path: string }) => `Retry “${p.path}”`,
  "knowledge.import.state.Done": "Done",
  "knowledge.import.state.Partial": "Partial",
  "knowledge.import.state.Failed": "Failed",
  "knowledge.import.state.Skipped": "Skipped",
  "knowledge.import.state.Running": "In progress",
  "knowledge.import.state.Pending": "Waiting",

  "knowledge.import.pick.title": "Add notes to the graph",
  "knowledge.import.pick.search": "Find a note or folder",
  "knowledge.import.pick.none": "Nothing found.",
  "knowledge.import.pick.folder": (p: { count: number }) => enN(p.count, "# file", "# files"),
  "knowledge.import.pick.chosen": (p: { count: number }) => enN(p.count, "# item chosen", "# items chosen"),
  "knowledge.import.pick.limit": (p: { max: number }) => `No more than ${p.max} items at a time.`,
  "knowledge.import.pick.next": "Next",
  "knowledge.import.pick.hint": "Only what you choose is sent. The whole vault cannot be added at once.",

  "knowledge.failure.ImportForbidden": "The vault root, a path outside the vault, or more than 200 items cannot be imported.",
  "knowledge.failure.ImportBusy": "An import into this vault is already running.",
  "knowledge.failure.ImportChanged": "The files have changed. Estimate again.",

  // ── Refusals in the graph context (not ai.failure.*) ───────────────
  "knowledge.failure.JournalFailed": "Could not write to the knowledge graph files or the sent log: they may be damaged. Nothing was changed.",
  "knowledge.failure.Forbidden": "The lesson or the graph folder is closed to the agent in the settings.",
  "knowledge.failure.LessonUnreadable": "The lesson or statement was not found, or the file cannot be read.",
  "knowledge.failure.Cancelled": "The graph is already being filled from this lesson.",
} as const satisfies Entries;
