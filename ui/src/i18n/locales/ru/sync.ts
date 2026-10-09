import { ruN, type Translation } from "../define.js";
import type { sync as en } from "../en/sync.js";

export const sync: Translation<typeof en> = {
  "sync.panel.title": "Синхронизация",
  "sync.cancelOperation": "Прервать",
  "sync.autoPrefix": "Снимок",

  "sync.command.openPanel": "Панель синхронизации",
  "sync.command.snapshot": "Сделать снимок",
  "sync.command.fileHistory": "История заметки",
  "sync.command.push": "Отправить снимки",
  "sync.command.pull": "Получить снимки",
  "sync.command.resolveConflicts": "Разрешить конфликты",
  "sync.command.clone": "Создать vault из удалённого репозитория",
  "sync.tree.fileHistory": "История файла",
  "sync.tree.conflict": "есть неразрешённый конфликт",

  "sync.indicator.label": (p) => `Синхронизация: ${p.state}`,
  "sync.indicator.open": "Открыть панель синхронизации",
  "sync.indicator.loading": "синхронизация…",
  "sync.indicator.offline": "нет сети",
  "sync.indicator.failed": "обмен не удался",
  "sync.indicator.conflicts": (p) => `конфликты: ${p.count}`,
  "sync.indicator.keeperDisputed": "спор хранителей",
  "sync.indicator.ignoreWarning": "нет правил исключения",
  "sync.indicator.syncthing": (p) => `Syncthing ${p.percent}%`,
  "sync.indicator.syncthingNoHistory": "Syncthing, история выключена",
  "sync.indicator.noHistory": "история выключена",
  "sync.indicator.historyLost": "история пропала",
  "sync.indicator.foreign": "чужой репозиторий",
  "sync.indicator.mirror": "история у хранителя",
  "sync.indicator.changes": (p) => ruN(p.count, "# изменение", "# изменения", "# изменений"),
  "sync.indicator.aheadBehind": (p) => `↑${p.ahead} ↓${p.behind}`,
  "sync.indicator.upToDate": "актуально",

  "sync.activity.enable": "включаем историю…",
  "sync.activity.snapshot": "делаем снимок…",
  "sync.activity.push": "отправляем…",
  "sync.activity.pull": "получаем…",
  "sync.activity.connect": "подключаемся…",
  "sync.activity.resolve": "применяем…",

  "sync.error.offline": "Нет связи: сервер не ответил. Локальные снимки целы.",
  "sync.error.timeout": "Сервер не ответил вовремя. Локальные снимки целы.",
  "sync.error.authRequired": "Сервер просит данные доступа: укажите токен или имя пользователя.",
  "sync.error.authRejected":
    "Сервер отклонил данные доступа: токен неверен, истёк или не даёт доступа к этому репозиторию.",
  "sync.error.notFound": "По этому адресу репозитория нет, либо данные доступа его не открывают.",
  "sync.error.rejectedNonFastForward":
    "На сервере есть снимки, которых нет на этом устройстве. Сначала получите их, затем отправьте.",
  "sync.error.corsBlocked":
    "Браузер не пустил запрос: этот сервер не разрешает обращения со страниц.",
  "sync.error.unsupportedScheme": (p) =>
    `Адреса вида «${p.scheme}» в этой сборке приложения недоступны.`,
  "sync.error.invalidRemoteUrl": "Это не похоже на адрес репозитория.",
  "sync.error.remoteChanged": (p) =>
    `Адрес удалённого репозитория  на этом устройстве не подтверждён: он пришёл вместе с настройками vault. Проверьте его и подключите — до этого туда ничего не отправляется.`,
  "sync.error.noRemote": "Удалённый репозиторий не подключён.",
  "sync.error.keeperElsewhere": (p) =>
    `Историю ведёт устройство «${p.device}». Снимок отсюда требует подтверждения.`,
  "sync.error.keeperDisputed":
    "Два устройства считают себя хранителем истории. Выберите хранителя, прежде чем делать снимки.",
  "sync.error.inProgressOperation":
    "В истории не завершена операция, начатая вне приложения (слияние, перенос…). Сначала завершите или отмените её средствами git.",
  "sync.error.pullConflictsPending":
    "В последнем получении остались неразрешённые конфликты. Разрешите их или отмените получение.",
  "sync.error.noHistory": "История для этого vault не включена.",
  "sync.error.historyExists": "У этого vault история уже есть.",
  "sync.error.foreignRepository":
    "Vault лежит внутри репозитория, созданного вне приложения; приложение в него не пишет.",
  "sync.error.stIgnoreNotConfirmed":
    "Папку доставляет Syncthing: решите, добавлять ли правила исключения, прежде чем включать историю.",
  "sync.error.authorRequired": "Укажите автора снимков: имя и адрес почты.",
  "sync.error.notFlushed": "Часть открытых заметок не удалось сохранить, поэтому снимок не сделан.",
  "sync.error.nothingToSnapshot": "Снимать нечего: изменения не выбраны.",
  "sync.error.conflictNotFound": "Этого конфликта уже нет.",
  "sync.error.targetNotEmpty": "Выбранная папка не пуста. Выберите пустую папку.",
  "sync.error.cancelled": "Отменено.",
  "sync.error.io": (p) => `Операция не удалась: ${p.detail}`,

  "sync.inProgress.merge": "Не завершено слияние, начатое вне приложения.",
  "sync.inProgress.rebase": "Не завершён перенос снимков, начатый вне приложения.",
  "sync.inProgress.cherryPick": "Не завершён перенос отдельного снимка, начатый вне приложения.",
  "sync.inProgress.revert": "Не завершена отмена снимка, начатая вне приложения.",
  "sync.inProgress.bisect": "Не завершён поиск снимка (bisect), начатый вне приложения.",
  "sync.inProgress.other": "Не завершена операция git, начатая вне приложения.",

  "sync.change.added": "добавлен",
  "sync.change.modified": "изменён",
  "sync.change.deleted": "удалён",
  "sync.change.renamed": (p) => `переименован из ${p.from}`,
  "sync.content.binary": (p) => `Двоичный файл, ${p.size}.`,
  "sync.content.missing": "файла нет",

  "sync.history.title": "История",
  "sync.history.absent":
    "История выключена. С историей приложение хранит снимки vault: версии можно сравнивать и возвращать.",
  "sync.history.absentSyncthing":
    "Папку доставляет Syncthing, история выключена. История добавит снимки, к которым можно вернуться; сначала приложение предложит правила, чтобы Syncthing не разносил её по устройствам.",
  "sync.history.enable": "Включить историю…",
  "sync.history.lost":
    "У этого vault была история, а теперь её нет: папку истории удалили или vault перенесли без неё.",
  "sync.history.createNew": "Начать новую историю…",
  "sync.history.restoreFromRemote": "Загрузить историю из удалённого репозитория",
  "sync.history.restoreFromRemoteHint":
    "Адрес удалённого репозитория записан в настройках vault — история, скорее всего, цела там. Файлы, которые разошлись с ней, станут конфликтами; молча ничего не перезапишется.",
  "sync.history.foreign": (p) =>
    `Vault лежит внутри репозитория ${p.root}, созданного вне приложения. Снимков в нём приложение не делает.`,
  "sync.history.mirror": (p) =>
    `Историю ведёт «${p.keeper}»; здесь она читается из копии удалённого репозитория.`,
  "sync.history.mirrorNever": "Копия ещё не загружена.",
  "sync.history.mirrorAt": (p) => `Копия обновлена ${p.time}.`,
  "sync.history.refreshMirror": "Обновить копию",
  "sync.history.inProgressHint": "Пока она не завершена, снимки недоступны.",
  "sync.history.snapshots": "Снимки",
  "sync.history.empty": "Снимков пока нет.",
  "sync.history.more": "Показать более ранние",
  "sync.history.open": "история",
  "sync.history.restoreDeleted": "восстановить",
  "sync.history.fileTitle": (p) => `История ${p.path}`,
  "sync.history.comparison": "Сравнение",
  "sync.history.pick": "Выберите снимок, чтобы увидеть, что в нём изменилось.",
  "sync.history.compareWith": "Сравнить снимок с",
  "sync.history.againstCurrent": "текущим файлом",
  "sync.history.againstPrevious": "предыдущим снимком",
  "sync.history.renamedHint": (p) =>
    `В этом снимке файл назывался ${p.name}; сравнить его можно только с предыдущим снимком.`,
  "sync.history.binary": (p) => `Двоичный файл: ${p.before} → ${p.after}.`,
  "sync.history.restore": "Вернуть эту версию",
  "sync.history.cannotRestore":
    "Эту версию отсюда не вернуть: в ней файл удалён, либо он назывался иначе и не открыт в редакторе.",
  "sync.history.notText": "эта версия — не текст",
  "sync.history.restoredEditor": "Версия в редакторе. Отмена вернёт прежний текст.",
  "sync.history.restoredFile": "Файл возвращён к этой версии. История не изменилась.",

  "sync.changes.label": "Изменения с последнего снимка",
  "sync.changes.none": "С последнего снимка ничего не изменилось.",
  "sync.changes.include": (p) => `Включить в снимок: ${p.path}`,
  "sync.changes.compare": "Сравнить с последним снимком",
  "sync.changes.binary": "Двоичный файл — построчно сравнивать нечего.",
  "sync.snapshot.message": "Сообщение снимка",
  "sync.snapshot.messagePlaceholder": "пусто — будут перечислены изменённые заметки",
  "sync.snapshot.all": "Сделать снимок",
  "sync.snapshot.selected": (p) => `Снять ${ruN(p.count, "# файл", "# файла", "# файлов")}`,

  "sync.ignore.stignoreMissing":
    "В правилах исключения Syncthing нет строк приложения: история и кэши могут уехать на другие устройства и испортиться.",
  "sync.ignore.gitignoreMissing": "В правилах исключения истории нет строк приложения.",
  "sync.ignore.fix": "Добавить правила",
  "sync.ignore.showRules": "Показать правила",
  "sync.ignore.hideRules": "Скрыть правила",
  "sync.ignore.open": "Открыть правила",
  "sync.rules.title": "Правила исключения",
  "sync.rules.explain":
    "Файлы правил лежат в корне vault. Здесь их можно открыть и изменить, не выходя из приложения.",
  "sync.rules.gitignore": "Правила истории",
  "sync.rules.gitignoreHint": "что не попадает в снимки",
  "sync.rules.stignore": "Правила Syncthing",
  "sync.rules.stignoreHint": "что не доставляется на другие устройства",
  "sync.rules.edit": "Открыть",
  "sync.rules.sharedGit": "Этот файл общий для всех устройств и сам попадает в снимки.",
  "sync.rules.perDeviceSt":
    "У каждого устройства этот файл свой: Syncthing его не доставляет, и правка действует только здесь.",
  "sync.rules.notInEffectGit":
    "Истории в этом vault сейчас нет — правила начнут действовать, когда она появится.",
  "sync.rules.notInEffectSt":
    "Syncthing в этой папке не обнаружен — правила начнут действовать, когда папка станет его папкой.",
  "sync.rules.foreignParent":
    "Vault лежит внутри другого репозитория: на файлы действуют и правила каталогов выше, здесь они не показаны.",
  "sync.rules.willCreate": "Файла ещё нет — он будет создан при сохранении.",
  "sync.rules.field": (p) => `Содержимое файла ${p.file}`,
  "sync.rules.blockLabel": "правила приложения",
  "sync.rules.blockHint":
    "Подсвеченные строки приложение ведёт само. Их можно менять, но без них служебные данные попадут в снимки или в доставку.",
  "sync.rules.cannot.NotText": "Файл не является текстом в UTF-8 — править его здесь нельзя.",
  "sync.rules.cannot.TooLarge": "Файл больше предела размера заметки — править его здесь нельзя.",
  "sync.rules.cannot.NotAFile":
    "Под этим именем — символическая ссылка или то, что не читается как файл. Правка недоступна.",
  "sync.rules.cannot.ReadOnly": "Vault открыт без права записи — сохранить правку нельзя.",
  "sync.rules.cannot.HistoryBusy":
    "У истории незавершённая операция. Завершите или отмените её — после этого правила можно сохранить.",
  "sync.rules.missing": "Не хватает правил приложения:",
  "sync.rules.riskStIgnore":
    "Без них служебный каталог истории и кэши уедут через Syncthing на другие устройства и испортятся.",
  "sync.rules.riskGitIgnore":
    "Без них в снимки попадут кэши, служебные файлы Syncthing и конфликтные копии.",
  "sync.rules.restore": "Вернуть правила приложения",
  "sync.rules.complete": "Правила приложения на месте.",
  "sync.rules.saved": "Сохранено.",
  "sync.rules.saving": "Сохранение…",
  "sync.rules.updatedOutside": "Файл изменился на диске — показана новая версия.",
  "sync.rules.staleOutside":
    "Файл изменился на диске, пока вы его правили. При сохранении приложение покажет обе версии.",
  "sync.rules.confirmTitle": "Сохранить без правил приложения?",
  "sync.rules.confirmSave": "Сохранить без них",
  "sync.rules.backToEdit": "Вернуться к правке",
  "sync.rules.divergedTitle": "Файл изменился на диске",
  "sync.rules.divergedExplain":
    "На диске не та версия, с которой вы начали правку. Ничего не записано. Строки «<» — ваши, «>» — с диска.",
  "sync.rules.deletedExplain":
    "Файл удалён с диска, пока вы его правили. Ничего не записано.",
  "sync.rules.takeDisk": "Взять версию с диска",
  "sync.rules.dropMine": "Отбросить мою правку",
  "sync.rules.overwrite": "Записать мою",
  "sync.rules.recreate": "Создать файл заново",
  "sync.rules.closeTitle": "Правка не сохранена",
  "sync.rules.closeExplain": (p) => `В файле ${p.file} есть несохранённые изменения.`,
  "sync.rules.discard": "Отбросить",

  "sync.wizard.title": "Включение истории",
  "sync.wizard.rulesLabel": "Правила, которые будут добавлены",
  "sync.wizard.stignoreIntro":
    "Эту папку доставляет Syncthing. Чтобы он не разносил историю и кэши по устройствам, приложение добавит в начало .stignore такие строки:",
  "sync.wizard.stignoreNote": (p) =>
    p.count > 0
      ? `Ваши ${ruN(p.count, "# строка", "# строки", "# строк")} в файле останутся нетронутыми.`
      : "Файл будет создан.",
  "sync.wizard.stignoreDeclineWarning":
    "Без правил Syncthing доставляет историю по одному файлу и может её испортить.",
  "sync.wizard.stignoreAccept": "Добавить правила",
  "sync.wizard.stignoreDecline": "Не добавлять",
  "sync.wizard.gitignoreIntro":
    "Кэшам и служебным файлам Syncthing в истории не место. Приложение добавит в конец .gitignore такие строки:",
  "sync.wizard.gitignoreNote": (p) =>
    p.count > 0
      ? `Ваши ${ruN(p.count, "# строка", "# строки", "# строк")} в файле останутся нетронутыми.`
      : "Файл будет создан.",
  "sync.wizard.gitignoreComplete": "Правила уже на месте — добавлять нечего.",
  "sync.wizard.next": "Дальше",
  "sync.wizard.authorIntro": "Каждый снимок подписан автором.",
  "sync.wizard.authorHint":
    "Оставьте оба поля пустыми, чтобы взять имя и адрес из общих настроек git.",
  "sync.wizard.enable": "Включить и сделать первый снимок",
  "sync.wizard.running": "Делаем первый снимок…",
  "sync.wizard.firstMessage": "Первый снимок",
  "sync.author.name": "Имя",
  "sync.author.email": "Адрес почты",

  "sync.remote.title": "Удалённый репозиторий",
  "sync.remote.none": "Не подключён. Удалённый репозиторий хранит копию истории вне этого устройства.",
  "sync.remote.needsHistory": "Сначала включите историю.",
  "sync.remote.connectEllipsis": "Подключить…",
  "sync.remote.connectTitle": "Подключение удалённого репозитория",
  "sync.remote.cloneTitle": "Vault из удалённого репозитория",
  "sync.remote.cloneHint":
    "После адреса приложение попросит пустую папку и загрузит в неё репозиторий.",
  "sync.remote.url": "Адрес репозитория",
  "sync.remote.urlHintSsh": "https://… с токеном или ssh-адрес — используются ключи вашей системы.",
  "sync.remote.urlHintNoSsh": "https://… с токеном. Доступ по ssh-ключам в этой сборке приложения недоступен.",
  "sync.remote.user": "Имя пользователя",
  "sync.remote.userHint": "Обычно не нужно: для известных хостингов приложение подставит его само.",
  "sync.remote.token": "Токен доступа",
  "sync.remote.tokenHintSecure": "Хранится в хранилище секретов системы, в vault не попадает.",
  "sync.remote.tokenHintBrowser":
    "Хранится в хранилище этого браузера, в vault не попадает. Защищённого хранилища у браузера нет: токен прочтёт любой, у кого есть доступ к этому профилю.",
  "sync.remote.proxy": "Адрес посредника",
  "sync.remote.proxyHint":
    "Браузер обменивается только с серверами, которые это разрешают. Для остальных укажите посредника, которому доверяете: через него пройдут токен и заметки.",
  "sync.remote.corsAdvice": "Укажите посредника или используйте настольное приложение.",
  "sync.remote.connect": "Подключить",
  "sync.remote.clone": "Выбрать папку и загрузить",
  "sync.remote.connecting": "Проверяем доступ…",
  "sync.remote.cloning": "Загружаем репозиторий…",
  "sync.remote.notCounted": "с сервером ещё не сверялись",
  "sync.remote.counts": (p) => `↑${p.ahead} к отправке · ↓${p.behind} к получению`,
  "sync.remote.neverExchanged": "обмена ещё не было",
  "sync.remote.exchangedAt": (p) => `последний обмен ${p.time}`,
  "sync.remote.pullPending": "Получение ждёт разрешения конфликтов.",
  "sync.remote.push": "Отправить",
  "sync.remote.pull": "Получить",
  "sync.remote.pullFirst": "Сначала получить",
  "sync.remote.replaceAccess": "Сменить адрес или данные доступа",
  "sync.remote.setProxy": "Указать посредника",
  "sync.remote.disconnect": "Отключить",
  "sync.remote.disconnectHint": "История и файлы остаются на месте; убирается только связь с сервером.",

  "sync.syncthing.title": "Syncthing",
  "sync.syncthing.detected": "Эту папку доставляет Syncthing.",
  "sync.syncthing.notDetected": "Эта папка — не папка Syncthing.",
  "sync.syncthing.link.notConfigured": "Чтение состояния Syncthing не настроено.",
  "sync.syncthing.link.unreachable": "Syncthing не отвечает по указанному адресу.",
  "sync.syncthing.link.unauthorized": "Syncthing отклонил ключ API.",
  "sync.syncthing.link.unavailableInShell":
    "В этой сборке приложения состояние Syncthing не читается: его интерфейс не отвечает на обращения со страниц.",
  "sync.syncthing.link.folderNotShared": "Syncthing отвечает, но этой папки в нём нет.",
  "sync.syncthing.link.certificateChanged":
    "Сертификат по этому адресу не тот, что был при первом подключении. Ключ API туда не отправляется. Если вы переустанавливали Syncthing, отключите чтение состояния и подключите заново.",
  "sync.syncthing.link.connected": "Состояние Syncthing читается.",
  "sync.syncthing.state.idle": "Всё доставлено",
  "sync.syncthing.state.scanning": "Сканирование",
  "sync.syncthing.state.syncing": "Идёт доставка",
  "sync.syncthing.state.paused": "Приостановлено",
  "sync.syncthing.state.error": "Ошибка",
  "sync.syncthing.progress": (p) => `доставлено ${p.percent}%`,
  "sync.syncthing.devices": "Устройства этой папки",
  "sync.syncthing.deviceConnected": "на связи",
  "sync.syncthing.deviceOffline": "не на связи",
  "sync.syncthing.find": "Найти Syncthing",
  "sync.syncthing.manual": "Указать вручную",
  "sync.syncthing.disconnect": "Не читать состояние",
  "sync.syncthing.notFound": "Настройки Syncthing на этом устройстве не найдены. Укажите адрес и ключ вручную.",
  "sync.syncthing.confirm": "Подключение к найденному Syncthing",
  "sync.syncthing.foundAt": (p) => `Syncthing найден по адресу ${p.address}.`,
  "sync.syncthing.confirmHint":
    "Приложение будет только читать состояние папки. Ключ API уйдёт в хранилище секретов этого устройства.",
  "sync.syncthing.connect": "Подключить",
  "sync.syncthing.address": "Адрес",
  "sync.syncthing.addressHint": "Только адрес этого устройства: ключ в сеть не отправляется.",
  "sync.syncthing.tls": "Подключаться по HTTPS",
  "sync.syncthing.apiKey": "Ключ API",

  "sync.conflicts.title": "Конфликты",
  "sync.conflicts.none": "Конфликтов нет.",
  "sync.conflicts.fromPull": "из получения",
  "sync.conflicts.fromSyncthing": (p) => `копия Syncthing с ${p.device}`,
  "sync.conflicts.autoClean": "совмещается без вопросов",

  "sync.merge.title": (p) => `Конфликт в ${p.path}`,
  "sync.merge.sourcePull": (p) =>
    `Версия с сервера (снимок ${p.snapshot}) расходится с вашей.`,
  "sync.merge.sourceSyncthing": (p) => `Syncthing сохранил копию с ${p.device} (${p.stamp}).`,
  "sync.merge.ours": "Своя",
  "sync.merge.base": "Общая",
  "sync.merge.theirs": "Их",
  "sync.merge.autoClean": "Версии совмещены автоматически — проверьте результат ниже.",
  "sync.merge.disputed": "Спорные участки",
  "sync.merge.disputedCount": (p) =>
    ruN(p.count, "# спорный участок", "# спорных участка", "# спорных участков"),
  "sync.merge.choose": "Чью версию взять",
  "sync.merge.takeOurs": "своя",
  "sync.merge.takeTheirs": "их",
  "sync.merge.takeBoth": "обе",
  "sync.merge.emptySide": "(пусто)",
  "sync.merge.result": "Результат",
  "sync.merge.manualHint": "Исправлено вручную: повторный выбор стороны пересоберёт результат.",
  "sync.merge.wholeOnly": "Построчно эти версии не совместить — выберите одну целиком.",
  "sync.merge.keepBothPath": "Имя для второй версии",
  "sync.merge.copySuffix": " (копия)",
  "sync.merge.keepBoth": "Оставить обе файлами",
  "sync.merge.keepOurs": "Оставить свою",
  "sync.merge.useTheirs": "Взять их",
  "sync.merge.saveMerged": "Сохранить результат",
  "sync.merge.abortPull": "Отменить получение",
  "sync.merge.abortPullHint": "Все файлы вернутся к состоянию до получения.",

  "sync.keeper.title": "Хранитель истории",
  "sync.keeper.unassigned": "Хранитель не назначен.",
  "sync.keeper.thisDevice": "Историю ведёт это устройство.",
  "sync.keeper.otherDevice": (p) => `Историю ведёт «${p.device}».`,
  "sync.keeper.disputed":
    "Два устройства считают себя хранителем. Автоснимки остановлены везде, пока вы не выберете одного.",
  "sync.keeper.unknown": "другое устройство",
  "sync.keeper.since": (p) => `назначен ${p.time}`,
  "sync.keeper.lastSnapshot": (p) => `последний снимок ${p.time}`,
  "sync.keeper.noSnapshots": "снимков ещё нет",
  "sync.keeper.explain":
    "Когда vault доставляет Syncthing, снимки делает одно устройство — хранитель. Остальные читают историю из удалённого репозитория.",
  "sync.keeper.take": "Сделать это устройство хранителем",
  "sync.keeper.clear": "Снять назначение",
  "sync.keeper.confirmTitle": "Это устройство — не хранитель",
  "sync.keeper.confirmElsewhere": (p) =>
    `Историю ведёт «${p.device}». Снимок или обмен отсюда может продублировать его снимки.`,
  "sync.keeper.confirmDisputed":
    "Два устройства считают себя хранителем. Снимок или обмен отсюда может продублировать снимки.",
  "sync.keeper.confirmHint": "Продолжайте, только если хранитель недоступен.",
  "sync.keeper.confirmProceed": "Всё равно продолжить",

  "sync.settings.title": "Настройки синхронизации",
  "sync.settings.auto": "Автоматика",
  "sync.settings.autoSnapshot": "Делать снимки автоматически",
  "sync.settings.autoSnapshotHint": "После паузы с последнего изменения, на этом устройстве — хранителе.",
  "sync.settings.autoSnapshotNotKeeper":
    "Работает только на хранителе истории; это устройство — не хранитель.",
  "sync.settings.pause": "Пауза после последнего изменения, секунд",
  "sync.settings.autoPush": "Отправлять после каждого снимка",
  "sync.settings.autoPullOnOpen": "Получать при открытии vault",
  "sync.settings.autoPullInterval": "Получать каждые, минут",
  "sync.settings.autoPullIntervalHint": "0 — по расписанию не получать.",
  "sync.settings.needsRemote": "Автообмену нужен подключённый удалённый репозиторий.",
  "sync.settings.author": "Автор снимков на этом устройстве",
  "sync.settings.authorHint": "Пусто — имя и адрес из общих настроек git.",
};
