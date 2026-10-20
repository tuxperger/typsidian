import { Prec, type Extension } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";

// Клавиши открытого списка дополнения (фича 018, FR-003). Работают только
// пока список открыт: иначе клавиша идёт своим путём — поля сниппета (003),
// перенос строки и отступ не страдают.

export interface CompletionKeysPort {
  isOpen(): boolean;
  move(delta: number): void;
  accept(): void;
  close(): void;
}

/** Расширение создаётся один раз на редактор; порт читается при нажатии. */
export function completionKeys(port: () => CompletionKeysPort | null): Extension {
  const when = (act: (open: CompletionKeysPort) => void) => (): boolean => {
    const current = port();
    if (current === null || !current.isOpen()) return false;
    act(current);
    return true;
  };
  return [
    Prec.highest(
      keymap.of([
      { key: "ArrowDown", run: when((open) => open.move(1)) },
      { key: "ArrowUp", run: when((open) => open.move(-1)) },
      { key: "Enter", run: when((open) => open.accept()) },
      { key: "Tab", run: when((open) => open.accept()) },
      { key: "Escape", run: when((open) => open.close()) },
      ]),
    ),
    // Фокус ушёл из редактора — список закрывается (выбор мышью фокус не
    // отнимает: строки списка гасят `pointerdown`).
    EditorView.domEventHandlers({
      blur: () => {
        port()?.close();
        return false;
      },
    }),
  ];
}
