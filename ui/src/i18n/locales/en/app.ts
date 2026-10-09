import { enN, type Entries } from "../define.js";

export const app = {
  "app.dependencyUnreadable": (p: { path: string; reason: string }) => `${p.path}: ${p.reason}`,
  "app.templateMissing": (p: { spec: string }) => `Template ${p.spec} is not installed in the notes folder`,
  "app.templateDownloaded": (p: { spec: string; url: string; fileCount: number; fingerprint: string }) =>
    `Template ${p.spec} downloaded from ${p.url} (${enN(p.fileCount, "# file", "# files")}, fingerprint ${p.fingerprint.slice(0, 12)}).`,
  "app.templateInstallFailed": (p: { spec: string; reason: string }) =>
    `Template ${p.spec} not installed: ${p.reason}`,
  "app.templateFingerprintDiverged": (p: { spec: string; recorded: string; actual: string }) =>
    `The content of template ${p.spec} diverged from its recorded fingerprint (${p.actual} instead of ${p.recorded.slice(0, 12)}); the app doesn’t reinstall it`,
  "app.vaultFontUnusable": (p: { path: string; reason: string }) => `Font ${p.path}: ${p.reason}`,
  "app.noteOutsideVault": (p: { path: string }) => `${p.path}: the note is outside the vault folder`,
  "app.buildTimedOut": (p: { seconds: number }) =>
    `The build was stopped: it ran longer than ${p.seconds} s. Check the document for an endless loop.`,
  "app.vaultUnavailable":
    "The vault folder is unavailable: linked files and fonts were not read, so the view may be incomplete. The text in the editor is kept.",
  "app.resolve.notFound": "file not found",
  "app.resolve.unreadable": "file can’t be read",
  "app.resolve.accessDenied": "no permission to read",
  "app.resolve.outsideVault": "file is outside the vault folder",
  "app.resolve.tooLarge": "file exceeds the supported size",
  "app.font.invalid": "the file can’t be parsed as a font",
  "app.fingerprint.digest": (p: { digest: string }) => p.digest.slice(0, 12),
  "app.fingerprint.fileMissing": (p: { name: string }) => `file ${p.name} is gone`,
  "app.install.offline": (p: { url: string }) => `No network: ${p.url} is unreachable.`,
  "app.install.sourceUnavailable": (p: { url: string }) => `Source ${p.url} refused the request.`,
  "app.install.blocked": (p: { url: string }) =>
    `Access to ${p.url} is blocked by this client’s environment. You can put the template by hand into this notes folder’s .typsidian/packages/.`,
  "app.install.versionNotFound": (p: { spec: string }) =>
    `The source has no such version (${p.spec}) — check the version in the import.`,
  "app.install.timedOut": (p: { url: string }) => `Waiting for a response from ${p.url} timed out.`,
  "app.install.archiveRejected": (p: { reason: string }) =>
    `The archive was rejected, no files were created: ${p.reason}.`,
  "app.install.notWritable": "The notes folder isn’t writable — nowhere to install the template.",
  "app.install.localWriteFailed": "The template files couldn’t be written to the notes folder.",
  "app.install.autoDownloadDisabled": (p: { spec: string }) =>
    `Template ${p.spec} isn’t installed, and auto-download is off.`,
  "app.install.noVaultOpen": "No notes folder is open.",
  "app.install.unknown": (p: { detail: string }) => `The template wasn’t installed: ${p.detail}`,
  "app.archive.tooLarge": "the archive exceeds the supported size",
  "app.archive.unparsable": "the archive can’t be parsed",
  "app.archive.entryWithoutPath": "an archive entry has no usable path",
  "app.archive.entryUnreadable": "an archive entry can’t be read",
  "app.archive.unpackedTooLarge": "the unpacked template exceeds the supported size",
  "app.archive.empty": "the archive has no files",
  "app.archive.noManifest": "the archive has no typst.toml manifest",
  "app.archive.pathNotText": "an archive entry path isn’t text",
  "app.archive.escapesFolder": (p: { entry: string }) => `entry “${p.entry}” leads outside the template folder`,
  "app.archive.entryWithoutName": "an archive entry has no name",
} as const satisfies Entries;
