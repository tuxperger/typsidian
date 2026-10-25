import { ruN, type Translation } from "../define.js";
import type { server as en } from "../en/server.js";

// Серверный режим (фича 037): стартовый экран, вход, свои vault, аккаунт, связь.
export const server: Translation<typeof en> = {
  "server.start.title": "Где лежат заметки?",
  "server.start.device": "Папка на устройстве",
  "server.start.deviceHint": "Заметки остаются в выбранной папке этого компьютера. Сервер не нужен.",
  "server.start.server": "Сервер",
  "server.start.serverHint": (p) => `Вход на свой сервер Typsidian: ${p.host}`,
  "server.start.serverUnavailable": "Сервер Typsidian по этому адресу не отвечает.",
  "server.start.continue": "Продолжить",
  "server.start.switch": "Войти на свой сервер…",

  "server.login.login": "Логин",
  "server.login.password": "Пароль",
  "server.login.submit": "Войти",
  "server.login.or": "или",
  "server.login.oidc": (p) => `Войти через ${p.label}`,
  "server.login.back": "← Папка на устройстве",
  "server.login.bad": "Неверный логин или пароль.",
  "server.login.tooManyTitle": "Слишком много попыток",
  "server.login.tooMany": (p) =>
    `Вход с логином «${p.login}» закрыт на ${ruN(p.minutes, "# минуту", "# минуты", "# минут")}.`,
  "server.login.unknownAccount": (p) =>
    `Аккаунт для этого пользователя ${p.label} не заведён — обратитесь к администратору сервера.`,
  "server.login.oidcFailed": "Вход через провайдера не завершился. Попробуйте ещё раз.",
  "server.login.expired": "Сессия завершена. Войдите снова — несохранённые правки сохранятся после входа.",
  "server.login.offline": "Нет связи с сервером.",
  "server.login.failed": "Не удалось войти.",

  "server.vaults.title": "Мои vault",
  "server.vaults.logout": "Выйти",
  "server.vaults.open": "Открыть",
  "server.vaults.more": "Ещё",
  "server.vaults.rename": "Переименовать",
  "server.vaults.delete": "Удалить",
  "server.vaults.notes": (p) => ruN(p.count, "# заметка", "# заметки", "# заметок"),
  "server.vaults.empty": "Vault пока нет — создайте первый ниже.",
  "server.vaults.newName": "Имя нового vault",
  "server.vaults.create": "Создать",
  "server.vaults.renameTo": "Новое имя",
  "server.vaults.save": "Сохранить",
  "server.vaults.cancel": "Отмена",
  "server.vaults.confirmDelete": (p) =>
    `Удалить vault «${p.name}» и ${ruN(p.count, "# заметку", "# заметки", "# заметок")} в нём?`,
  "server.vaults.confirmDeleteHint":
    "Папка уйдёт в корзину сервера — администратор сможет её вернуть.",
  "server.vaults.confirmDeleteButton": "Удалить vault",
  "server.vaults.badName": "В имени vault нельзя / \\ : * ? \" < > |, точку в начале или в конце, длиннее 100 знаков.",
  "server.vaults.taken": "Vault с таким именем уже есть.",
  "server.vaults.failed": "Сервер не выполнил действие.",
  "server.vaults.switch": "Мои vault",

  "server.account.section": "Аккаунт",
  "server.account.via": (p) => `${p.host} · вход паролем`,
  "server.account.viaOidc": (p) => `${p.host} · вход через провайдера`,
  "server.account.changePassword": "Сменить пароль",
  "server.account.current": "Текущий пароль",
  "server.account.next": "Новый пароль",
  "server.account.repeat": "Новый пароль ещё раз",
  "server.account.save": "Сохранить пароль",
  "server.account.tooShort": "Не короче 12 знаков.",
  "server.account.mismatch": "Новые пароли не совпали.",
  "server.account.wrong": "Текущий пароль неверный.",
  "server.account.changed": "Пароль сменён. Остальные сессии завершены.",
  "server.account.sessions": "Входы",
  "server.account.thisSession": "этот",
  "server.account.lastSeen": (p) =>
    p.ip === "" ? `последнее действие — ${p.when}` : `последнее действие — ${p.when} · ${p.ip}`,
  "server.account.now": "сейчас",
  "server.account.end": "Завершить",
  "server.account.logoutAll": "Выйти на всех устройствах",
  "server.account.unknownBrowser": "Браузер",

  "server.connection.offline": "Нет связи с сервером — правки сохранятся, когда связь вернётся.",
  "server.connection.retryIn": (p) => `Повтор через ${p.seconds} с`,
  "server.connection.retry": "Повторить",
  "server.demo.try": "Попробовать без регистрации",
  "server.demo.terms": (p) =>
    `Свой vault с примерами на ${p.minutes} мин. Ничего вводить не нужно; потом vault и все правки удаляются.`,
  "server.demo.starting": "Готовим ваш vault…",
  "server.demo.tooMany": (p) =>
    `С этого адреса за последний час начато слишком много демо. Попробуйте через ${p.minutes} мин.`,
  "server.demo.failed": "Демо не запустилось. Попробуйте чуть позже.",
  "server.demo.busy": "Сейчас в демо слишком много посетителей. Попробуйте через несколько минут.",
  "server.demo.left": (p) => `демо · осталось ${p.minutes} мин`,
  "server.demo.leftClock": (p) => `демо · осталось ${p.clock}`,
  "server.demo.over": "демо · время вышло",
  "server.demo.usage": (p) => `Демо-объём: ${p.used} из ${p.quota} МБ`,
  "server.demo.full": (p) =>
    `Превышен демо-объём: ${p.quota} МБ. Удалите крупные вложения или файлы экспорта, чтобы сохранить снова.`,
  "server.demo.soon": (p) =>
    `Демо закончится через ${p.clock}. Правки после этого не сохранятся; нужное скачайте экспортом.`,
  "server.demo.soonOk": "Понятно",
  "server.demo.endedTitle": "Демо закончилось",
  "server.demo.ended": (p) =>
    `Прошло ${p.minutes} мин: ваш vault и все правки удалены с сервера. Можно начать заново — с чистой копией примеров.`,
  "server.demo.again": "Начать заново",
};
