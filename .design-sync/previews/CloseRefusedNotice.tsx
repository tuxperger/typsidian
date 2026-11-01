import { CloseRefusedNotice } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const r = (x: any) => ({ tab: 1, path: "Результаты.typ", title: "Результаты", detail: null, ...x });
export const Unsaved = () => <Frame><CloseRefusedNotice refusal={r({ reason: "Unsaved" }) as any} onDismiss={noop} /></Frame>;
export const Conflict = () => <Frame><CloseRefusedNotice refusal={r({ reason: "Conflict" }) as any} onDismiss={noop} /></Frame>;
