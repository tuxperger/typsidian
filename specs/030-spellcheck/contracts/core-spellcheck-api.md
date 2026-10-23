# Contract: ядро → клиенты (проверка орфографии)

Ядро (`packages/core/src/spellcheck/`):

```text
SpellChecker::default()                       // включено, ru+en, слов vault нет
spell_configure(backend, &mut SpellChecker) -> SpellConfig
spell_add_word(backend, &mut SpellChecker, word) -> Result<SpellConfig, SpellError>
spell_remove_word(backend, &mut SpellChecker, word) -> Result<SpellConfig, SpellError>
spell_suggest(&SpellChecker, word) -> Vec<String>          // ≤ 5
HighlightSession::spelling(&self, windows, &SpellChecker) -> Vec<u32>   // пары UTF-16
misspelled_in(text, windows, &SpellChecker) -> Vec<u32>    // без сессии, для тестов и бенча
```

`SpellError`: `InvalidWord`, `ReadOnly`, `Io`.

wasm: класс `SpellChecker` (`new`, `configure(backend)`, `addWord`, `removeWord`,
`suggest`), `SyntaxSession.spelling(windows, checker)`.

Tauri: состояние `Speller`; команды `spell_configure`, `spell_add_word`,
`spell_remove_word`, `spell_suggest`, `syntax_session_spelling { id, windows }`.

Клиент (`ui/src/vault-client.ts`):

```ts
interface SpellingClient {
  configure(): Promise<SpellConfig>;
  addWord(word: string): Promise<SpellConfig>;   // отказ — исключение с кодом
  removeWord(word: string): Promise<SpellConfig>;
  suggest(word: string): Promise<string[]>;
}
interface HighlightSession { spelling(windows): Promise<Uint32Array | undefined>; … }
```
