import { VaultSwitcher } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const device: any = { snapshot: async () => ({ settings: { current_vault: "v1" }, knownVaults: [{ id: "v1", location: "/home/anna/диссертация-2026" }, { id: "v2", location: "/home/anna/лекции" }] }) };
export const Closed = () => <Frame><VaultSwitcher rootName="диссертация-2026" device={device} onOpenKnown={noop} onOpenFolder={noop} onManage={noop} /></Frame>;
