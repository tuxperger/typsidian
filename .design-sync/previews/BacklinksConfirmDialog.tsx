import { BacklinksConfirmDialog } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const note = (name: string) => ({ path: name, name, stamp: {}, has_unsaved_edits: false, kind: "Note" }) as any;
export const Delete = () => <Frame h="100vh"><BacklinksConfirmDialog request={{ operation: "delete", note: note("Методология.typ"), affected: [note("Введение.typ"), note("Результаты.typ"), note("Лекция_12.typ")] }} onConfirm={noop} onCancel={noop} /></Frame>;
