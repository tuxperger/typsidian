import { CompletionPopup } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const items: any = [
  { label: "fig:thermogram", insert: "fig:thermogram", matched: [], preview: "@fig:thermogram", detail: { kind: "Label", label_kind: "Figure", text: "Термограмма образца", file: "Результаты.typ", duplicate: false } },
  { label: "sec:results", insert: "sec:results", matched: [], preview: "@sec:results", detail: { kind: "Label", label_kind: "Heading", text: "Результаты", file: null, duplicate: false } },
  { label: "smirnov2023", insert: "smirnov2023", matched: [], preview: "@smirnov2023", detail: { kind: "Source", summary: "Смирнов А. Тепловые волны, 2023" } },
];
const session: any = { items, active: 0, anchor: { left: 40, top: 40, bottom: 58 }, list: { groups: [{ kind: "Labels", total: 2, items: items.slice(0, 2) }, { kind: "Sources", total: 1, items: items.slice(2) }] } };
export const List = () => <Frame h="100vh"><CompletionPopup session={session} narrow={false} editorBox={() => null} onPick={noop} onHover={noop} /></Frame>;
