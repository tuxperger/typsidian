import { TabStrip } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const tab = (id: number, title: string, x: any = {}) => ({ id, group: "First", path: title + ".typ", title, kind: "Note", unsaved: false, conflict: false, active: false, ...x });
const tabs = [tab(1, "Введение"), tab(2, "Результаты", { active: true }), tab(3, "Смирнов_2023", { unsaved: true }), tab(4, "Обсуждение", { conflict: true })];
const shortcuts: any = { bindings: [], matches: () => false, displayOf: () => "Ctrl+W" };
const base: any = { slot: "First", tabs, activeId: 2, canBack: true, canForward: false, split: false, showSplitControl: true, canSplit: true, canUnsplit: false, canMoveToOther: false, shortcuts, onActivate: noop, onClose: noop, onCloseOthers: noop, onBack: noop, onForward: noop, onCycle: noop, onToggleSplit: noop, onMoveToOther: noop, onMove: noop };
export const Focused = () => <Frame><TabStrip {...base} focused /></Frame>;
export const Unfocused = () => <Frame><TabStrip {...base} focused={false} /></Frame>;
