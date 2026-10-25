import { enN, type Entries } from "../define.js";

// Server mode (feature 037): start screen, sign-in, own vaults, account, connection.
export const server = {
  "server.start.title": "Where are your notes?",
  "server.start.device": "Folder on this device",
  "server.start.deviceHint": "Notes stay in a folder you choose on this computer. No server needed.",
  "server.start.server": "Server",
  "server.start.serverHint": (p: { host: string }) => `Sign in to your Typsidian server: ${p.host}`,
  "server.start.serverUnavailable": "No Typsidian server answers at this address.",
  "server.start.continue": "Continue",
  "server.start.switch": "Sign in to your server…",

  "server.login.login": "Login",
  "server.login.password": "Password",
  "server.login.submit": "Sign in",
  "server.login.or": "or",
  "server.login.oidc": (p: { label: string }) => `Sign in with ${p.label}`,
  "server.login.back": "← Folder on this device",
  "server.login.bad": "Wrong login or password.",
  "server.login.tooManyTitle": "Too many attempts",
  "server.login.tooMany": (p: { login: string; minutes: number }) =>
    `Sign-in as “${p.login}” is closed for ${enN(p.minutes, "# minute", "# minutes")}.`,
  "server.login.unknownAccount": (p: { label: string }) =>
    `No account is set up for this ${p.label} user — ask the server administrator.`,
  "server.login.oidcFailed": "Sign-in through the provider did not complete. Try again.",
  "server.login.expired": "Your session has ended. Sign in again — unsaved edits will be saved after you do.",
  "server.login.offline": "The server is not reachable.",
  "server.login.failed": "Could not sign in.",

  "server.vaults.title": "My vaults",
  "server.vaults.logout": "Sign out",
  "server.vaults.open": "Open",
  "server.vaults.more": "More",
  "server.vaults.rename": "Rename",
  "server.vaults.delete": "Delete",
  "server.vaults.notes": (p: { count: number }) => enN(p.count, "# note", "# notes"),
  "server.vaults.empty": "No vaults yet — create the first one below.",
  "server.vaults.newName": "New vault name",
  "server.vaults.create": "Create",
  "server.vaults.renameTo": "New name",
  "server.vaults.save": "Save",
  "server.vaults.cancel": "Cancel",
  "server.vaults.confirmDelete": (p: { name: string; count: number }) =>
    `Delete vault “${p.name}” and ${enN(p.count, "the # note", "the # notes")} in it?`,
  "server.vaults.confirmDeleteHint":
    "The folder goes to the server trash — the administrator can restore it.",
  "server.vaults.confirmDeleteButton": "Delete vault",
  "server.vaults.badName": "A vault name cannot contain / \\ : * ? \" < > |, start or end with a dot, or exceed 100 characters.",
  "server.vaults.taken": "A vault with this name already exists.",
  "server.vaults.failed": "The server did not complete the action.",
  "server.vaults.switch": "My vaults",

  "server.account.section": "Account",
  "server.account.via": (p: { host: string }) => `${p.host} · sign-in with a password`,
  "server.account.viaOidc": (p: { host: string }) => `${p.host} · sign-in through the provider`,
  "server.account.changePassword": "Change password",
  "server.account.current": "Current password",
  "server.account.next": "New password",
  "server.account.repeat": "New password again",
  "server.account.save": "Save password",
  "server.account.tooShort": "At least 12 characters.",
  "server.account.mismatch": "The new passwords do not match.",
  "server.account.wrong": "The current password is wrong.",
  "server.account.changed": "Password changed. Other sessions have been ended.",
  "server.account.sessions": "Sign-ins",
  "server.account.thisSession": "this one",
  "server.account.lastSeen": (p: { when: string; ip: string }) =>
    p.ip === "" ? `last activity — ${p.when}` : `last activity — ${p.when} · ${p.ip}`,
  "server.account.now": "now",
  "server.account.end": "End",
  "server.account.logoutAll": "Sign out everywhere",
  "server.account.unknownBrowser": "Browser",

  "server.connection.offline": "No connection to the server — edits will be saved when it returns.",
  "server.connection.retryIn": (p: { seconds: number }) => `Retrying in ${p.seconds} s`,
  "server.connection.retry": "Retry",
  "server.demo.try": "Try without signing up",
  "server.demo.terms": (p: { minutes: number }) =>
    `Your own vault with examples for ${p.minutes} min. Nothing to enter; afterwards the vault and all edits are deleted.`,
  "server.demo.starting": "Preparing your vault…",
  "server.demo.tooMany": (p: { minutes: number }) =>
    `Too many demos have been started from this address in the last hour. Try again in ${p.minutes} min.`,
  "server.demo.failed": "The demo could not be started. Try again a bit later.",
  "server.demo.busy": "The demo has too many visitors right now. Try again in a few minutes.",
  "server.demo.left": (p: { minutes: number }) => `demo · ${p.minutes} min left`,
  "server.demo.leftClock": (p: { clock: string }) => `demo · ${p.clock} left`,
  "server.demo.over": "demo · time is up",
  "server.demo.usage": (p: { used: string; quota: string }) =>
    `Demo storage: ${p.used} of ${p.quota} MB`,
  "server.demo.full": (p: { quota: number }) =>
    `Demo storage is full: ${p.quota} MB. Delete large attachments or exported files to save again.`,
  "server.demo.soon": (p: { clock: string }) =>
    `The demo ends in ${p.clock}. Edits after that will not be saved; download what you need with export.`,
  "server.demo.soonOk": "Got it",
  "server.demo.endedTitle": "The demo has ended",
  "server.demo.ended": (p: { minutes: number }) =>
    `${p.minutes} min have passed: your vault and all edits have been deleted from the server. You can start again with a fresh copy of the examples.`,
  "server.demo.again": "Start again",
} satisfies Entries;
