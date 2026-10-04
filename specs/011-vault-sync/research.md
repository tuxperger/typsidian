# Research: Синхронизация vault (011)

Каждый раздел — решение, обоснование, отвергнутые альтернативы. Ссылки на FR — на
`spec.md` этой фичи.

## §1. Движок git: внешний движок за трейтом ядра

**Decision**: Git — внешний движок по принципу VIII. Ядро владеет трейтом `GitEngine`
(низкоуровневые операции: объекты, индекс, ссылки, транспорт) и **всей политикой**:
какие файлы снимать, правила исключения, хранитель, автоснимки, трёхстороннее слияние,
откат, восстановление после отмены. Реализации:

| Таргет | Реализация | Где живёт |
| --- | --- | --- |
| native (desktop, Android) | `git2` (libgit2, `vendored-libgit2`, `default-features = false`) | `packages/core/src/sync/engine/native.rs` |
| wasm32 (web) | isomorphic-git в JS, передаётся в ядро объектом, реализующим импортированный интерфейс | `packages/core/src/sync/engine/web.rs` + `ui/src/sync/git-engine-web.ts` |

**Rationale**:
- gitoxide (`gix`) — чистый Rust, но **push в нём нет** (на 2026-10 это пункт дорожной
  карты к 1.0; сторонний `gunnar_sendpack` закрывает пробел, но молод), и сам `gix` под
  `wasm32-unknown-unknown` не собирается (issue #463, discussion #2211: «not ready for
  WASM»). Значит, единого Rust-движка на оба таргета нет — нужен шов (принцип II разрешает
  cfg только внутри реализаций за трейтом).
- libgit2 зрелый, умеет всё из FR-001…FR-020 (status, commit выбранных путей, log, blob по
  ревизии, merge-base, fetch, push) и собирается под Android NDK вендорной копией.
- isomorphic-git — единственный зрелый git для браузера: свой `fs`-адаптер, свой `http`,
  параметр `corsProxy` прямо соответствует уточнению про посредника (FR-037).
- Слияние, откат и запись файлов делает **ядро**, а не движок: ядро читает три версии
  блоба и пишет результат через `VaultBackend` (атомарная запись 009, события 001). Поэтому
  поведение конфликтов и отмены одинаково на обоих таргетах, а движку остаются только
  объекты и сеть.

**Alternatives considered**:
- *Системный `git` CLI на desktop* — нет на Android и в браузере; разный вывод разных
  версий; отвергнут.
- *isomorphic-git везде (и в Tauri через webview)* — снимает C-зависимость, но SSH он не
  умеет (FR-014a требует ключи на desktop), а status на 5 000 файлах в JS заметно медленнее
  libgit2 (SC-002). Отвергнут как основной, остаётся для web.
- *Собственный git в ядре поверх `gix-object`/`gix-pack` (feature `wasm`)* — пришлось бы
  писать индекс, протокол и сборку pack для push; объём несоразмерен фиче.

## §2. Транспорт native без OpenSSL

**Decision**: `git2` собирается без встроенных `https`/`ssh`. Через
`git2::transport::register` регистрируются два своих subtransport:
- `https://` (и `http://` только для loopback) — smart HTTP поверх уже имеющегося в ядре
  `reqwest` с `rustls-tls` (`GET …/info/refs?service=…`, `POST …/git-upload-pack|receive-pack`),
  тайм-ауты из §9. Токен — заголовком `Authorization: Basic` (`x-access-token:<token>` /
  `oauth2:<token>` — пользователь вводит токен, имя подставляется по хосту, для прочих
  серверов — поле «имя пользователя»).
- `ssh://` и `user@host:path` — **только desktop**: процесс системного `ssh` (`ssh -o
  BatchMode=yes host git-upload-pack '<path>'`), stdin/stdout — канал протокола. Ключи,
  агент, `known_hosts`, `~/.ssh/config` — пользователя, ничего не храним (FR-014a). На
  Android `ssh`-адрес отклоняется при подключении (FR-014a).

**Rationale**: конституция ядра уже избегает системного OpenSSL на NixOS (комментарий к
`reqwest` в `Cargo.toml`); `libssh2`/`openssl-sys` потребовали бы его или вендорную сборку
под NDK. Один HTTP-клиент на весь native — те же тайм-ауты и прокси, что у загрузки
шаблонов (007). Субпроцесс `ssh` даёт ровно ту конфигурацию, что пользователь уже настроил.

**Alternatives**: `git2` с `https`+`ssh` (OpenSSL, libssh2 — новые нативные зависимости в
flake и в Android-сборке); `git2-curl` (curl — ещё одна C-зависимость).

## §3. Движок в web: isomorphic-git поверх папки пользователя

**Decision**: `ui/src/sync/git-engine-web.ts` — тонкий адаптер (без политики):
- `fs`-адаптер isomorphic-git (`promises.readFile/writeFile/unlink/readdir/mkdir/rmdir/
  stat/lstat`) поверх того же `FileSystemDirectoryHandle`, что держит `WebBackend`; `.git`
  лежит в папке пользователя (FR-002), читается стандартным git.
- `http` — `isomorphic-git/http/web`; `corsProxy` — адрес посредника из настроек
  устройства (FR-037), по умолчанию пусто.
- Объект адаптера передаётся в `WebSyncSession::new(engine)`; ядро вызывает его через
  `#[wasm_bindgen] extern "C" { type JsGitEngine; … }` (методы возвращают `Promise`).
  Типы аргументов/результатов — из Rust через `tsify` (принцип IV).

**Rationale**: принцип VIII — движок чёрный ящик, ядро владеет файлом и политикой.
Обращения в браузере к GitHub/GitLab без CORS невозможны — это ограничение платформы,
оно показывается как отказ с причиной (FR-036, FR-037), а не скрывается.

**Alternatives**: wasm-сборка libgit2 через emscripten — несовместима с
`wasm32-unknown-unknown`/wasm-bindgen; `lightning-fs` (IndexedDB) — история жила бы не в
папке пользователя, нарушая FR-002 и принцип VI.

## §4. Модель «хранитель истории» (Clarifications)

**Decision**:
- Назначение хранится в `.typsidian/vault.json` (доставляется Syncthing вместе с vault):
  `sync.keeper = { device_id, device_name, since }`. Идентификатор устройства — новое
  значение в настройках устройства (`device.id`, случайный 128-битный, создаётся при первом
  запуске; `device.name` — имя хоста, редактируемо).
- Хранитель держит `.git` в папке vault. Остальные устройства `.git` не имеют (Syncthing его
  не доставляет — §5) и для FR-033 ведут **зеркало** удалённого репозитория в каталоге
  устройства (native: каталог данных приложения `mirrors/<vault-id>.git`, bare; web: OPFS
  `mirrors/<vault-id>`), обновляемое `fetch` при открытии истории.
- Конфликт назначения: появление конфликтной копии `vault.json` или расхождение `keeper` с
  тем, что видело устройство при последнем снимке, → состояние `KeeperDisputed`, автоснимки
  стоп на всех устройствах (FR-032) до явного выбора.

**Rationale**: `.git` нельзя доставлять Syncthing — пофайловая доставка без транзакций
рвёт согласованность ссылок и объектов. Свой клон на каждом устройстве порождает
дублирующиеся снимки одних правок и `pull`, упирающийся в изменения, уже доставленные
Syncthing (отвергнуто в Clarifications).

## §5. Правила исключения Syncthing (`.stignore`)

**Decision**: модуль ядра `sync::stignore`:
- Syncthing **не доставляет** `.stignore` между устройствами — у каждого свой. Поэтому
  проверка (FR-026) идёт на каждом устройстве при открытии vault.
- Обязательные правила: `.git`, производные пути служебного места
  (`service_dir::derived_paths()`: `.typsidian/cache.sqlite*`, `.typsidian/tmp`). Настройки
  (`vault.json`) и `packages/` — доставляются.
- Добавление — блок с маркерами-комментариями (`// typsidian:begin` … `// typsidian:end`)
  в **начало** файла: в `.stignore` выигрывает первое совпавшее правило, и правило
  пользователя `!…` ниже не должно отменить исключение `.git`. Повторное применение
  заменяет блок целиком (FR-027: без дублей), строки вне блока не трогаются.
- Запись — атомарная через `VaultBackend` (009 research §4).

**Alternatives**: `#include .stglobalignore` (общий доставляемый файл правил) — удобно для
пользователя с многими правилами, но всё равно требует строки `#include` в `.stignore`
каждого устройства; оставлено как возможное улучшение.

## §6. Правила исключения истории (`.gitignore`)

**Decision**: при включении истории (FR-001, FR-003) ядро предлагает блок в `.gitignore`
с теми же маркерами (`# typsidian:begin/end`): `.typsidian/cache.sqlite*`, `.typsidian/tmp/`,
`*.sync-conflict-*`, `.stversions/`, `.stfolder/`, `.stignore`. Настройки vault, доски,
вложения, `packages/` — в истории. Существующий `.gitignore` (как в `/home/apokryf/notes`:
`result*`, `*.pdf`) сохраняется.

## §7. Конфликтные копии Syncthing

**Decision**: `sync::conflict_copy::parse(name)` распознаёт
`<stem>.sync-conflict-<YYYYMMDD>-<HHMMSS>-<DEVID7>[.<ext>]` (формат Syncthing ≥ 0.14) и
возвращает исходное имя, время, короткий идентификатор устройства. Общая функция
`backend::is_index_excluded(path)` (сейчас `native::is_excluded`/`web::is_excluded` —
обе получают проверку из одного места, принцип II) исключает такие файлы из индекса,
поиска и графа (FR-024). Отдельный обход `VaultBackend::list_conflict_copies()` и события
вотчера пополняют список конфликтов. Имя устройства-источника — из списка устройств
Syncthing, если состояние читается (§8), иначе короткий идентификатор.

## §8. Состояние Syncthing

**Decision**: модуль `sync::syncthing` (только чтение, FR-023) — REST API Syncthing:
`GET /rest/system/status` (проверка ключа), `/rest/config/folders` (найти папку по
пути = корень vault), `/rest/db/status?folder=` (state, needBytes/globalBytes → прогресс),
`/rest/system/connections` (подключённые устройства), `/rest/config/devices` (имена),
`/rest/events?events=FolderSummary,StateChanged,DeviceConnected,DeviceDisconnected&since=`
(long-poll 60 с) для обновления без действий пользователя (FR-022). HTTP — тот же `reqwest`.
- **Поиск параметров на desktop (FR-022a)**: `config.xml` Syncthing в стандартных местах
  (`$XDG_STATE_HOME/syncthing`, `~/.local/state/syncthing`, `~/.config/syncthing`,
  macOS `~/Library/Application Support/Syncthing`, Windows `%LOCALAPPDATA%\Syncthing`):
  `<gui><address>`, `<apikey>`, `tls`. Разбор — `quick-xml` (уже в `Cargo.lock`
  транзитивно). Найденное предлагается одним подтверждением; ключ уходит в хранилище
  секретов (§10).
- **Android**: Syncthing-Fork слушает `127.0.0.1:8384` по HTTPS с самоподписанным
  сертификатом; ключ вводится вручную. Для loopback-адреса сертификат закрепляется при
  первом подключении (TOFU, отпечаток в настройках устройства).
- **Web**: GUI Syncthing не отдаёт CORS — состояние «неизвестно» с объяснением (FR-036).
- Хранитель: если папка не `idle`/не синхронизирована, автоснимок откладывается (FR-031).

**Alternatives**: опрос `db/status` каждые N секунд — проще, но лишний трафик и задержка;
оставлен как запасной путь при ошибке long-poll.

## §9. Сеть: тайм-ауты, офлайн, атомарность

**Decision**: соединение 10 с, отсутствие данных 20 с, общий предел операции обмена
30 с без прогресса (SC-004). Отмена — общий механизм массовых операций (`export::batch`,
принцип X). `pull` выполняется как: fetch (в объекты, файлы не трогаются) → план слияния в
ядре → запись файлов через `VaultBackend` с журналом исходных версий → обновление индекса
и `HEAD`. Отмена/ошибка до шага «обновление HEAD» → откат файлов из журнала (FR-016, Edge
Cases). Push не трогает рабочие файлы.

## §10. Хранилище секретов

**Decision**: трейт ядра `SecretStore { get, set, delete }`, ключ — `(vault_id, kind)`:
- desktop: крейт `keyring` 3 (Linux — Secret Service через чистый Rust `zbus`-бэкенд
  `sync-secret-service` + `crypto-rust`; macOS Keychain; Windows Credential Manager);
- Android: файл в приватном каталоге приложения (`app_data_dir`, недоступен другим
  приложениям), не в vault;
- web: IndexedDB origin-а приложения. Браузер не даёт защищённого хранилища — это
  указывается в интерфейсе при вводе токена.
Секреты не попадают в `vault.json`, `.git/config` (URL хранится без учётных данных; URL с
`user:token@` при подключении очищается и токен переносится в хранилище), сообщения и
журналы (FR-014, SC-008).

## §11. Трёхстороннее слияние текста

**Decision**: `sync::merge::merge3(base, ours, theirs) -> Merged | Conflicts` — diff3 по
строкам поверх диффа `similar` (чистый Rust, собирается под wasm32, Myers/Patience).
Непересекающиеся ханки применяются автоматически (SC-007), пересекающиеся — участки
конфликта для ручного совмещения в UI. Двоичные файлы (не UTF-8 или NUL в первых 8 КБ) —
только выбор версии целиком. Существующий LCS из `vault::conflict` (бюджет строк для
диалога внешнего изменения) не меняется: у него другая задача.

## §12. Автоснимки и таймеры

**Decision**: политика — чистая функция ядра
`autosnapshot::decide(now, last_change_at, last_snapshot_at, keeper, syncthing_state,
pause) -> Decision { Wait(until) | Snapshot | Blocked(reason) }`. Таймер — у оболочки
(Tauri — `tokio::time` в `packages/shell`; web — `setTimeout` в `ui/src/sync/`), который
лишь вызывает `sync_tick()`. Так тестируется вся логика без часов, а платформенные таймеры
остаются связующим слоем.

## §13. Производительность статуса (SC-002)

**Decision**: libgit2 status с включённым кэшем индекса (`stat`-проверка) на 5 000 файлах
укладывается в сотни мс; в web isomorphic-git `statusMatrix` ограничивается путями,
изменёнными по событиям вотчера с момента последнего полного прохода (полный проход — при
открытии панели в фоне, с прогрессом). Бенчмарк `benches/sync_status.rs` на 5 000 заметок.
