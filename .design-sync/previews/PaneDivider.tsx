import { PaneDivider } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const pane = (text: string, flex: number) => <div style={{ flex, background: "var(--steel-900)", padding: 12, fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--steel-400)" }}>{text}</div>;
export const BetweenPanes = () => <Frame><div style={{ display: "flex", height: 120, border: "1px solid var(--steel-800)" }}><div style={{ flex: 1, display: "flex", borderRight: "1px solid var(--steel-800)" }}>{pane("Дерево vault", 1)}</div><PaneDivider label="Ширина дерева" now={248} min={180} max={420} onDrag={noop} onStep={noop} onReset={noop} />{pane("Редактор", 2)}</div></Frame>;
