import { MoveNotice } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

export const Undone = () => <Frame><MoveNotice notice={{ kind: "undone" } as any} busy={false} onUndo={noop} onDismiss={noop} /></Frame>;
export const Failed = () => <Frame><MoveNotice notice={{ kind: "failed", message: () => "В папке «Литература» уже есть файл с таким именем." } as any} busy={false} onUndo={noop} onDismiss={noop} /></Frame>;
