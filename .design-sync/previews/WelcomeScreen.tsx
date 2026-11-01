import { WelcomeScreen } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const device: any = { snapshot: async () => ({ settings: { current_vault: "v1" }, knownVaults: [{ id: "v1", location: "/home/anna/диссертация-2026" }, { id: "v2", location: "/home/anna/лекции" }, { id: "v3", location: "/home/anna/статья-тепло" }] }) };
export const WithRecent = () => <Frame h="100vh"><WelcomeScreen isLoading={false} error={null} device={device} onOpenFolder={noop} onOpenKnown={noop} onClone={noop} onSettings={noop} /></Frame>;
export const FirstRun = () => <Frame h="100vh"><WelcomeScreen isLoading={false} error={null} device={undefined} onOpenFolder={noop} onOpenKnown={undefined} onClone={noop} onSettings={noop} /></Frame>;
export const Opening = () => <Frame h="100vh"><WelcomeScreen isLoading error={null} device={undefined} onOpenFolder={noop} onOpenKnown={undefined} onClone={undefined} onSettings={undefined} /></Frame>;
