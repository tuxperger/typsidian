import { enN, type Entries } from "../define.js";

export const sync = {
  "sync.panel.title": "Sync",
  "sync.cancelOperation": "Cancel operation",
  "sync.autoPrefix": "Snapshot",

  "sync.command.openPanel": "Sync panel",
  "sync.command.snapshot": "Take a snapshot",
  "sync.command.fileHistory": "Note history",
  "sync.command.push": "Push snapshots",
  "sync.command.pull": "Pull snapshots",
  "sync.command.resolveConflicts": "Resolve conflicts",
  "sync.command.clone": "Create a vault from a remote repository",
  "sync.tree.fileHistory": "File history",
  "sync.tree.conflict": "has an unresolved conflict",

  "sync.indicator.label": (p: { state: string }) => `Sync: ${p.state}`,
  "sync.indicator.open": "Open the sync panel",
  "sync.indicator.loading": "sync…",
  "sync.indicator.offline": "offline",
  "sync.indicator.failed": "sync failed",
  "sync.indicator.conflicts": (p: { count: number }) => `conflicts: ${p.count}`,
  "sync.indicator.keeperDisputed": "keeper disputed",
  "sync.indicator.ignoreWarning": "exclusion rules missing",
  "sync.indicator.syncthing": (p: { percent: number }) => `Syncthing ${p.percent}%`,
  "sync.indicator.syncthingNoHistory": "Syncthing, history off",
  "sync.indicator.noHistory": "history off",
  "sync.indicator.historyLost": "history is gone",
  "sync.indicator.foreign": "someone else’s repository",
  "sync.indicator.mirror": "history on the keeper",
  "sync.indicator.changes": (p: { count: number }) => enN(p.count, "# change", "# changes"),
  "sync.indicator.aheadBehind": (p: { ahead: number; behind: number }) =>
    `↑${p.ahead} ↓${p.behind}`,
  "sync.indicator.upToDate": "up to date",

  "sync.activity.enable": "enabling history…",
  "sync.activity.snapshot": "taking a snapshot…",
  "sync.activity.push": "pushing…",
  "sync.activity.pull": "pulling…",
  "sync.activity.connect": "connecting…",
  "sync.activity.resolve": "applying…",

  "sync.error.offline": "No network connection: the server did not answer. Local snapshots are safe.",
  "sync.error.timeout": "The server did not answer in time. Local snapshots are safe.",
  "sync.error.authRequired": "The server asks for credentials: specify a token or a user name.",
  "sync.error.authRejected":
    "The server rejected the credentials: the token is wrong, expired or lacks access to this repository.",
  "sync.error.notFound": "No repository at this address, or the credentials do not give access to it.",
  "sync.error.rejectedNonFastForward":
    "The server has snapshots this device does not have. Pull them first, then push.",
  "sync.error.corsBlocked":
    "The browser blocked the request: this server does not allow requests from web pages.",
  "sync.error.unsupportedScheme": (p: { scheme: string }) =>
    `Addresses of the “${p.scheme}” kind are not available in this app build.`,
  "sync.error.internalRemote":
    "The address leads into the server’s internal network — the server does not use such repositories unless the administrator allows them.",
  "sync.error.invalidRemoteUrl": "This does not look like a repository address.",
  "sync.error.remoteChanged": (p: { url: string }) =>
    `The remote address  has not been confirmed on this device: it came with the vault settings. Check it and connect — until then nothing is sent there.`,
  "sync.error.noRemote": "No remote repository is connected.",
  "sync.error.keeperElsewhere": (p: { device: string }) =>
    `History is kept by the device “${p.device}”. Snapshots from here need a confirmation.`,
  "sync.error.keeperDisputed":
    "Two devices claim to be the history keeper. Choose the keeper before taking snapshots.",
  "sync.error.exchangeRunning": "An exchange with the remote repository is already running — wait for it to finish.",
  "sync.error.inProgressOperation":
    "An operation started outside the app (merge, rebase…) is unfinished in the history. Finish or abort it with git first.",
  "sync.error.pullConflictsPending":
    "The last pull has unresolved conflicts. Resolve them or cancel the pull.",
  "sync.error.noHistory": "History is not enabled for this vault.",
  "sync.error.historyExists": "This vault already has a history.",
  "sync.error.foreignRepository":
    "The vault lies inside a repository created outside the app; the app does not write into it.",
  "sync.error.stIgnoreNotConfirmed":
    "Syncthing delivers this folder: decide about the exclusion rules before enabling history.",
  "sync.error.authorRequired": "Specify the author of snapshots: a name and an email address.",
  "sync.error.notFlushed": "Some open notes could not be saved, so the snapshot was not taken.",
  "sync.error.nothingToSnapshot": "Nothing to snapshot: no changes selected.",
  "sync.error.conflictNotFound": "This conflict no longer exists.",
  "sync.error.targetNotEmpty": "The chosen folder is not empty. Pick an empty folder.",
  "sync.error.cancelled": "Cancelled.",
  "sync.error.io": (p: { detail: string }) => `The operation failed: ${p.detail}`,
  "sync.error.tooLarge": (p: { limit: number }) =>
    `A file in the history or the upload is larger than ${p.limit} MB — it can’t be processed in memory.`,

  "sync.inProgress.merge": "An unfinished merge started outside the app.",
  "sync.inProgress.rebase": "An unfinished rebase started outside the app.",
  "sync.inProgress.cherryPick": "An unfinished cherry-pick started outside the app.",
  "sync.inProgress.revert": "An unfinished revert started outside the app.",
  "sync.inProgress.bisect": "An unfinished bisect started outside the app.",
  "sync.inProgress.other": "An unfinished git operation started outside the app.",

  "sync.change.added": "added",
  "sync.change.modified": "modified",
  "sync.change.deleted": "deleted",
  "sync.change.renamed": (p: { from: string }) => `renamed from ${p.from}`,
  "sync.content.binary": (p: { size: string }) => `Binary file, ${p.size}.`,
  "sync.content.missing": "no file",

  "sync.history.title": "History",
  "sync.history.absent":
    "History is off. With history the app keeps snapshots of the vault: you can compare versions and return to any of them.",
  "sync.history.absentSyncthing":
    "Syncthing delivers this folder, history is off. History adds snapshots you can return to; the app will first suggest rules so that Syncthing does not carry the history to other devices.",
  "sync.history.enable": "Enable history…",
  "sync.history.lost":
    "This vault had a history, and now it is gone: the history folder was deleted or the vault was moved without it.",
  "sync.history.createNew": "Start a new history…",
  "sync.history.restoreFromRemote": "Load the history from the remote repository",
  "sync.history.restoreFromRemoteHint":
    "The remote repository address is stored in the vault settings, so the history is most likely intact there. Files that differ from it become conflicts; nothing is overwritten silently.",
  "sync.history.foreign": (p: { root: string }) =>
    `The vault lies inside the repository ${p.root}, created outside the app. The app does not take snapshots in it.`,
  "sync.history.mirror": (p: { keeper: string }) =>
    `History is kept by “${p.keeper}”; here it is read from a copy of the remote repository.`,
  "sync.history.mirrorNever": "The copy has not been downloaded yet.",
  "sync.history.mirrorAt": (p: { time: string }) => `Copy updated ${p.time}.`,
  "sync.history.refreshMirror": "Update the copy",
  "sync.history.inProgressHint": "Snapshots are unavailable until it is finished.",
  "sync.history.snapshots": "Snapshots",
  "sync.history.empty": "No snapshots yet.",
  "sync.history.more": "Show earlier",
  "sync.history.open": "history",
  "sync.history.restoreDeleted": "restore",
  "sync.history.fileTitle": (p: { path: string }) => `History of ${p.path}`,
  "sync.history.comparison": "Comparison",
  "sync.history.pick": "Choose a snapshot to see what changed in it.",
  "sync.history.compareWith": "Compare the snapshot with",
  "sync.history.againstCurrent": "the current file",
  "sync.history.againstPrevious": "the previous snapshot",
  "sync.history.renamedHint": (p: { name: string }) =>
    `In this snapshot the file was called ${p.name}; it can only be compared with the previous snapshot.`,
  "sync.history.binary": (p: { before: string; after: string }) =>
    `Binary file: ${p.before} → ${p.after}.`,
  "sync.history.restore": "Return to this version",
  "sync.history.cannotRestore":
    "This version cannot be returned from here: the file is deleted in it, or it had another name and is not open in the editor.",
  "sync.history.notText": "this version is not text",
  "sync.history.restoredEditor": "The version is in the editor. Undo brings the previous text back.",
  "sync.history.restoredFile": "The file is back to this version. History is unchanged.",

  "sync.changes.label": "Changes since the last snapshot",
  "sync.changes.none": "No changes since the last snapshot.",
  "sync.changes.include": (p: { path: string }) => `Include in the snapshot: ${p.path}`,
  "sync.changes.compare": "Compare with the last snapshot",
  "sync.changes.binary": "Binary file — nothing to compare line by line.",
  "sync.snapshot.message": "Snapshot message",
  "sync.snapshot.messagePlaceholder": "empty — the changed notes will be listed",
  "sync.snapshot.all": "Take a snapshot",
  "sync.snapshot.selected": (p: { count: number }) =>
    `Snapshot ${enN(p.count, "# file", "# files")}`,

  "sync.ignore.stignoreMissing":
    "Syncthing’s exclusion rules lack the app’s lines: the history and caches may travel to other devices and get corrupted.",
  "sync.ignore.gitignoreMissing": "The history’s exclusion rules lack the app’s lines.",
  "sync.ignore.fix": "Add the rules",
  "sync.ignore.showRules": "Show the rules",
  "sync.ignore.hideRules": "Hide the rules",
  "sync.ignore.open": "Open the rules",
  "sync.rules.title": "Exclusion rules",
  "sync.rules.explain":
    "The rule files live in the vault root. You can open and edit them here without leaving the app.",
  "sync.rules.gitignore": "History rules",
  "sync.rules.gitignoreHint": "what stays out of snapshots",
  "sync.rules.stignore": "Syncthing rules",
  "sync.rules.stignoreHint": "what is not delivered to other devices",
  "sync.rules.edit": "Open",
  "sync.rules.sharedGit": "This file is shared by all devices and is itself part of snapshots.",
  "sync.rules.perDeviceSt":
    "Every device has its own copy: Syncthing does not deliver this file, so the edit applies only here.",
  "sync.rules.notInEffectGit":
    "This vault has no history right now — the rules take effect once it does.",
  "sync.rules.notInEffectSt":
    "Syncthing was not detected in this folder — the rules take effect once it becomes a Syncthing folder.",
  "sync.rules.foreignParent":
    "The vault sits inside another repository: rules from the folders above also apply and are not shown here.",
  "sync.rules.willCreate": "The file does not exist yet — it will be created on save.",
  "sync.rules.field": (p: { file: string }) => `Contents of ${p.file}`,
  "sync.rules.blockLabel": "app rules",
  "sync.rules.blockHint":
    "The highlighted lines are maintained by the app. You may change them, but without them service data ends up in snapshots or in delivery.",
  "sync.rules.cannot.NotText": "The file is not UTF-8 text — it cannot be edited here.",
  "sync.rules.cannot.TooLarge": "The file exceeds the note size limit — it cannot be edited here.",
  "sync.rules.cannot.NotAFile":
    "This name is a symbolic link or something that cannot be read as a file. Editing is unavailable.",
  "sync.rules.cannot.ReadOnly": "The vault is open without write access — the edit cannot be saved.",
  "sync.rules.cannot.HistoryBusy":
    "The history has an unfinished operation. Finish or abort it — then the rules can be saved.",
  "sync.rules.missing": "App rules are missing:",
  "sync.rules.riskStIgnore":
    "Without them the history folder and caches travel through Syncthing to other devices and get corrupted.",
  "sync.rules.riskGitIgnore":
    "Without them caches, Syncthing service files and conflict copies end up in snapshots.",
  "sync.rules.restore": "Restore the app rules",
  "sync.rules.complete": "The app rules are in place.",
  "sync.rules.saved": "Saved.",
  "sync.rules.saving": "Saving…",
  "sync.rules.updatedOutside": "The file changed on disk — the new version is shown.",
  "sync.rules.staleOutside":
    "The file changed on disk while you were editing. On save the app will show both versions.",
  "sync.rules.confirmTitle": "Save without the app rules?",
  "sync.rules.confirmSave": "Save without them",
  "sync.rules.backToEdit": "Back to editing",
  "sync.rules.divergedTitle": "The file changed on disk",
  "sync.rules.divergedExplain":
    "The version on disk is not the one you started from. Nothing was written. Lines marked “<” are yours, “>” are from disk.",
  "sync.rules.deletedExplain":
    "The file was deleted from disk while you were editing. Nothing was written.",
  "sync.rules.takeDisk": "Take the version from disk",
  "sync.rules.dropMine": "Discard my edit",
  "sync.rules.overwrite": "Write mine",
  "sync.rules.recreate": "Create the file again",
  "sync.rules.closeTitle": "The edit is not saved",
  "sync.rules.closeExplain": (p: { file: string }) => `${p.file} has unsaved changes.`,
  "sync.rules.discard": "Discard",

  "sync.wizard.title": "Enable history",
  "sync.wizard.rulesLabel": "Rules to be added",
  "sync.wizard.stignoreIntro":
    "Syncthing delivers this folder. So that it does not carry the history and caches to other devices, the app will add these lines to the beginning of .stignore:",
  "sync.wizard.stignoreNote": (p: { count: number }) =>
    p.count > 0
      ? `Your own ${enN(p.count, "# line", "# lines")} in the file stay untouched.`
      : "The file will be created.",
  "sync.wizard.stignoreDeclineWarning":
    "Without the rules Syncthing delivers the history file by file and may corrupt it.",
  "sync.wizard.stignoreAccept": "Add the rules",
  "sync.wizard.stignoreDecline": "Do not add",
  "sync.wizard.gitignoreIntro":
    "Caches and Syncthing’s service files do not belong in the history. The app will add these lines to the end of .gitignore:",
  "sync.wizard.gitignoreNote": (p: { count: number }) =>
    p.count > 0
      ? `Your own ${enN(p.count, "# line", "# lines")} in the file stay untouched.`
      : "The file will be created.",
  "sync.wizard.gitignoreComplete": "The rules are already in place — nothing to add.",
  "sync.wizard.next": "Next",
  "sync.wizard.authorIntro": "Every snapshot is signed by its author.",
  "sync.wizard.authorHint":
    "Leave both fields empty to use the name and address from git’s global settings.",
  "sync.wizard.enable": "Enable and take the first snapshot",
  "sync.wizard.running": "Taking the first snapshot…",
  "sync.wizard.firstMessage": "First snapshot",
  "sync.author.name": "Name",
  "sync.author.email": "Email address",

  "sync.remote.title": "Remote repository",
  "sync.remote.none": "Not connected. A remote repository keeps a copy of the history outside this device.",
  "sync.remote.needsHistory": "Enable history first.",
  "sync.remote.connectEllipsis": "Connect…",
  "sync.remote.connectTitle": "Connect a remote repository",
  "sync.remote.cloneTitle": "Create a vault from a remote repository",
  "sync.remote.cloneHint":
    "After you specify the address, the app will ask for an empty folder and download the repository into it.",
  "sync.remote.url": "Repository address",
  "sync.remote.urlHintSsh": "https://… with a token, or an ssh address — the keys of your system are used.",
  "sync.remote.urlHintNoSsh": "https://… with a token. Access by ssh keys is not available in this app build.",
  "sync.remote.urlHintServer":
    "https://… with a token. Only external HTTPS addresses work on the server: ssh, folders and addresses of the server’s internal network are not used.",
  "sync.remote.user": "User name",
  "sync.remote.userHint": "Usually not needed: for well-known hosts the app substitutes it itself.",
  "sync.remote.token": "Access token",
  "sync.remote.tokenHintSecure": "Stored in the system’s secret storage, never in the vault.",
  "sync.remote.tokenHintServer": "Stored on the server for your account, never in the vault or the browser.",
  "sync.remote.tokenHintBrowser":
    "Stored in this browser’s storage, never in the vault. The browser has no protected storage: anyone with access to this profile can read the token.",
  "sync.remote.proxy": "Intermediary address",
  "sync.remote.proxyHint":
    "A browser can only talk to servers that allow it. For the others, specify an intermediary you trust: the token and notes pass through it.",
  "sync.remote.corsAdvice": "Specify an intermediary, or use the desktop app.",
  "sync.remote.connect": "Connect",
  "sync.remote.clone": "Choose a folder and download",
  "sync.remote.connecting": "Checking access…",
  "sync.remote.cloning": "Downloading the repository…",
  "sync.progress": (p: { operation: string; done: number; total: number }) =>
    `${p.operation}: ${p.done} of ${p.total}`,
  "sync.progress.firstSnapshot": "first snapshot",
  "sync.progress.pull": "receiving",
  "sync.progress.push": "sending",
  "sync.progress.clone": "downloading",
  "sync.remote.notCounted": "not compared with the server yet",
  "sync.remote.counts": (p: { ahead: number; behind: number }) =>
    `↑${p.ahead} to push · ↓${p.behind} to pull`,
  "sync.remote.neverExchanged": "no exchange yet",
  "sync.remote.exchangedAt": (p: { time: string }) => `last exchange ${p.time}`,
  "sync.remote.pullPending": "A pull is waiting for conflicts to be resolved.",
  "sync.remote.push": "Push",
  "sync.remote.pull": "Pull",
  "sync.remote.pullFirst": "Pull first",
  "sync.remote.replaceAccess": "Change address or credentials",
  "sync.remote.setProxy": "Specify an intermediary",
  "sync.remote.disconnect": "Disconnect",
  "sync.remote.disconnectHint": "History and files stay in place; only the link to the server is removed.",

  "sync.syncthing.title": "Syncthing",
  "sync.syncthing.detected": "Syncthing delivers this folder.",
  "sync.syncthing.notDetected": "This folder is not a Syncthing folder.",
  "sync.syncthing.link.notConfigured": "Reading Syncthing’s state is not set up.",
  "sync.syncthing.link.unreachable": "Syncthing does not answer at the specified address.",
  "sync.syncthing.link.unauthorized": "Syncthing rejected the API key.",
  "sync.syncthing.link.unavailableInShell":
    "Syncthing’s state cannot be read in this app build: its interface does not answer requests from web pages.",
  "sync.syncthing.link.unavailableOnServer":
    "The vault is on a server: Syncthing is not supported here. History and the remote repository work.",
  "sync.syncthing.link.folderNotShared": "Syncthing answers, but it does not know this folder.",
  "sync.syncthing.link.certificateChanged":
    "The certificate at this address is not the one seen at the first connection. The API key is not sent there. If you reinstalled Syncthing, stop reading the state and connect again.",
  "sync.syncthing.link.connected": "Syncthing’s state is being read.",
  "sync.syncthing.state.idle": "Up to date",
  "sync.syncthing.state.scanning": "Scanning",
  "sync.syncthing.state.syncing": "Delivering",
  "sync.syncthing.state.paused": "Paused",
  "sync.syncthing.state.error": "Error",
  "sync.syncthing.progress": (p: { percent: number }) => `${p.percent}% delivered`,
  "sync.syncthing.devices": "Devices of this folder",
  "sync.syncthing.deviceConnected": "connected",
  "sync.syncthing.deviceOffline": "offline",
  "sync.syncthing.find": "Find Syncthing",
  "sync.syncthing.manual": "Specify manually",
  "sync.syncthing.disconnect": "Stop reading the state",
  "sync.syncthing.notFound": "Syncthing’s settings were not found on this device. Specify the address and key manually.",
  "sync.syncthing.confirm": "Connect to the found Syncthing",
  "sync.syncthing.foundAt": (p: { address: string }) => `Syncthing found at ${p.address}.`,
  "sync.syncthing.confirmHint":
    "The app will only read the folder’s state. The API key goes to this device’s secret storage.",
  "sync.syncthing.connect": "Connect",
  "sync.syncthing.address": "Address",
  "sync.syncthing.addressHint": "Only an address on this device: the key is never sent over the network.",
  "sync.syncthing.tls": "Connect over HTTPS",
  "sync.syncthing.apiKey": "API key",

  "sync.conflicts.title": "Conflicts",
  "sync.conflicts.none": "No conflicts.",
  "sync.conflicts.fromPull": "from the pull",
  "sync.conflicts.fromSyncthing": (p: { device: string }) => `Syncthing copy from ${p.device}`,
  "sync.conflicts.autoClean": "merges without questions",

  "sync.merge.title": (p: { path: string }) => `Conflict in ${p.path}`,
  "sync.merge.sourcePull": (p: { snapshot: string }) =>
    `The version from the server (snapshot ${p.snapshot}) differs from yours.`,
  "sync.merge.sourceSyncthing": (p: { device: string; stamp: string }) =>
    `Syncthing kept a copy from ${p.device} (${p.stamp}).`,
  "sync.merge.ours": "Yours",
  "sync.merge.base": "Common",
  "sync.merge.theirs": "Theirs",
  "sync.merge.autoClean": "The versions were merged automatically — check the result below.",
  "sync.merge.disputed": "Disputed fragments",
  "sync.merge.disputedCount": (p: { count: number }) =>
    enN(p.count, "# disputed fragment", "# disputed fragments"),
  "sync.merge.choose": "Which version to take",
  "sync.merge.takeOurs": "yours",
  "sync.merge.takeTheirs": "theirs",
  "sync.merge.takeBoth": "both",
  "sync.merge.emptySide": "(nothing)",
  "sync.merge.result": "Result",
  "sync.merge.manualHint": "Edited by hand: choosing a side again will rebuild the result.",
  "sync.merge.wholeOnly": "These versions cannot be merged line by line — choose one of them entirely.",
  "sync.merge.keepBothPath": "Name for the second version",
  "sync.merge.copySuffix": " (copy)",
  "sync.merge.keepBoth": "Keep both as files",
  "sync.merge.keepOurs": "Keep yours",
  "sync.merge.useTheirs": "Take theirs",
  "sync.merge.saveMerged": "Save the result",
  "sync.merge.abortPull": "Cancel the pull",
  "sync.merge.abortPullHint": "All files return to how they were before the pull.",

  "sync.keeper.title": "History keeper",
  "sync.keeper.unassigned": "No keeper is assigned.",
  "sync.keeper.thisDevice": "This device keeps the history.",
  "sync.keeper.otherDevice": (p: { device: string }) => `History is kept by “${p.device}”.`,
  "sync.keeper.disputed":
    "Two devices claim to be the keeper. Automatic snapshots are stopped everywhere until you choose one.",
  "sync.keeper.unknown": "another device",
  "sync.keeper.since": (p: { time: string }) => `assigned ${p.time}`,
  "sync.keeper.lastSnapshot": (p: { time: string }) => `last snapshot ${p.time}`,
  "sync.keeper.noSnapshots": "no snapshots yet",
  "sync.keeper.explain":
    "When Syncthing delivers the vault, one device takes the snapshots — the keeper. The others read the history from the remote repository.",
  "sync.keeper.take": "Make this device the keeper",
  "sync.keeper.clear": "Remove the assignment",
  "sync.keeper.confirmTitle": "This device is not the keeper",
  "sync.keeper.confirmElsewhere": (p: { device: string }) =>
    `History is kept by “${p.device}”. A snapshot or exchange from here may duplicate its snapshots.`,
  "sync.keeper.confirmDisputed":
    "Two devices claim to be the keeper. A snapshot or exchange from here may duplicate snapshots.",
  "sync.keeper.confirmHint": "Proceed only if the keeper is unavailable.",
  "sync.keeper.confirmProceed": "Proceed anyway",

  "sync.settings.title": "Sync settings",
  "sync.settings.auto": "Automation",
  "sync.settings.autoSnapshot": "Take snapshots automatically",
  "sync.settings.autoSnapshotHint": "After the pause since the last change, on this device — the keeper.",
  "sync.settings.autoSnapshotNotKeeper":
    "Works only on the history keeper; this device is not the keeper.",
  "sync.settings.pause": "Pause after the last change, seconds",
  "sync.settings.autoPush": "Push after each snapshot",
  "sync.settings.autoPullOnOpen": "Pull when the vault opens",
  "sync.settings.autoPullInterval": "Pull every, minutes",
  "sync.settings.autoPullIntervalHint": "0 — do not pull on a schedule.",
  "sync.settings.needsRemote": "Automatic exchange needs a connected remote repository.",
  "sync.settings.author": "Snapshot author on this device",
  "sync.settings.authorHint": "Empty — the name and address from git’s global settings.",
} satisfies Entries;
