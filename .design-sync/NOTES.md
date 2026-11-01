# Заметки /design-sync

- У `ui` нет общей точки входа. Её задаёт `.design-sync/ds-entry.js` с компонентами без прямой привязки к vault и wasm; `buildCmd` копирует её в `ui/dist` и склеивает CSS скриптом `flatten-css.mjs`. Без этого шага сборка падает: `ui/dist` не в git.
- Конвертер не копирует относительные `@import` токенов из cssEntry, а `tokensGlob` работает только вместе с `tokensPkg`. Поэтому шрифты, токены и `app.css` склеены в `ui/dist/styles/ds.css`.
- Шрифты в приложении приходят из `next/font`, в бандле это удалённый `@import` Google Fonts (`.design-sync/fonts.css`).
- Запуск: `env -u LD_LIBRARY_PATH nix develop -c sh -c 'DS_CHROMIUM_PATH=$CHROME_PATH node .ds-sync/resync.mjs ...'`. Playwright ставится в `.ds-sync` с `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1`, браузер берётся из dev-оболочки.
- Имена экспортов в превью только латиницей с заглавной: карточка отбрасывает остальные (`/^[A-Z]/`), и превью становится пустым.
- Оверлеи нельзя растягивать через `position: fixed`: обёртка карточки трансформирована, диалог обрезается. Используется `minHeight: 100vh`. Серая полоса вокруг диалогов на снимках `?story=` идёт от отступа захвата, а не от компонента.
- `IconSprite` исключён из карточек (`componentSrcMap: null`), но есть в бандле: `Icon` без него пуст.

## Не показано в превью

- `AccessStateBanner` с `unavailable`: полноэкранный оверлей, в карточку с другими состояниями не помещается.
- `MoveNotice` с `done`, `SyncIndicator` со сводкой ядра: требуют данных ядра.
- `VaultSwitcher` с открытым меню и `PaneDivider` при наведении: состояния по действию пользователя.

## Риски при следующей синхронизации

- `dtsPropsFor` у `ViewSwitch`, `VaultSwitcher`, `WelcomeScreen` написан вручную (встроенные типы пропсов не извлекаются): при смене пропсов обновить.
- Моки данных в `previews/` (`TabView`, `CompletionItem`, снимок устройства) не проверяются типами: могут отстать от ядра.
- `device` в `dtsPropsFor` упрощён до нужного превью.
- Новые компоненты сами не попадут: их нужно добавить в `ds-entry.js`, `ds-entry.d.ts` и `componentSrcMap`.
- `buildCmd` пересобирает `ui/dist`; `ds.css` зависит от списка токенов в `flatten-css.mjs`.
