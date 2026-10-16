# Contract: канал между окном редактора и окнами показа

`ui/src/slides/show-channel.ts`. Окно редактора — владелец (research.md §7); окна показа и
докладчика — зрители. Реализации: `show-channel-web.ts` (`BroadcastChannel`),
`show-channel-tauri.ts` (`emitTo`/`listen`), в памяти — для Android (`ShowOverlay`).

## Интерфейс

```ts
export type ShowRole = "show" | "presenter";

export interface ShowHost {
  /** Открывает окно роли; false — окно не открылось (блокировщик, отказ ОС). */
  open(role: ShowRole): Promise<boolean>;
  close(role: ShowRole): Promise<void>;
  send(message: HostMessage): void;
  onMessage(handler: (message: ViewerMessage) => void): () => void;
  /** Что умеет оболочка: есть ли отдельные окна и вид докладчика (FR-009). */
  readonly supports: { windows: boolean; presenter: boolean };
}

export interface ShowViewer {
  send(message: ViewerMessage): void;
  onMessage(handler: (message: HostMessage) => void): () => void;
}
```

`ShowHost` выбирает оболочка и передаёт в `Workspace` пропом, как `openVaultFolder`;
`Workspace` по платформе не ветвится.

## Сообщения

Типы полей — из `wasm-core` (`ShowState`, `SlideLabel`, `PageSlideInfo`, `ShowKeymap`,
`Diagnostic`).

| Направление | `kind` | Поля | Когда |
|---|---|---|---|
| зритель → хозяин | `ready` | `role`, `have: string[]` | окно загрузилось; `have` — отпечатки в его кэше |
| хозяин → зритель | `hello` | `keymap`, `locale` | ответ на `ready` |
| хозяин → зритель | `deck` | `digests: string[]`, `svgs: Record<digest, string>`, `sizes: {w,h}[]`, `slides: PageSlideInfo[]` | после `ready` и после каждой удачной сборки; `svgs` — только новые для зрителя (research.md §10) |
| хозяин → зритель | `state` | `state: ShowState`, `label: SlideLabel`, `stale: boolean`, `failure?: Diagnostic` | после каждого изменения |
| хозяин → зритель | `end` | — | показ завершён; окно закрывается |
| зритель → хозяин | `action` | `action: ShowAction` | листание, переход по номеру, лента |
| зритель → хозяин | `exit` | — | Esc, кнопка «Выйти», «Завершить показ» |
| зритель → хозяин | `closed` | `role` | окно закрыто ОС (research.md §8) |
| зритель → хозяин | `togglePresenter` | — | кнопка «Докладчик» в окне показа |
| зритель → хозяин | `revealError` | — | «К ошибке» на плашке A4: выход из показа и каретка на первую ошибку |

Порядок: хозяин отвечает на каждый `ready` сообщениями `hello`, `deck`, `state`. Зритель
рисует, только получив все три; до того — фон показа.

## Гарантии

- Зритель не меняет `ShowState` сам: новое состояние приходит только в `state`.
- `closed` окна показа = `exit` (spec Edge Cases). `closed` вида докладчика только закрывает его.
- Потеря второго монитора: окно ОС остаётся на оставшемся, канал не рвётся (spec Edge Cases).
