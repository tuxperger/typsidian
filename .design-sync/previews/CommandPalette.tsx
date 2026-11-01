import { CommandPalette } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const note = (name: string) => ({ path: name, name, stamp: {}, has_unsaved_edits: false, kind: "Note" }) as any;
const commands: any = [{ id: "export", title: "Экспорт в PDF", meta: "Ctrl+Shift+P", icon: "download", run: noop }, { id: "graph", title: "Открыть граф связей", meta: "Ctrl+G", icon: "network", run: noop }, { id: "new", title: "Создать заметку", meta: "Ctrl+N", icon: "plus", run: noop }];
export const Open = () => <Frame h="100vh"><CommandPalette notes={[note("Результаты.typ"), note("Введение.typ"), note("Смирнов_2023.typ")]} commands={commands} onOpenNote={noop} onClose={noop} /></Frame>;
