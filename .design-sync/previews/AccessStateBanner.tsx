import { AccessStateBanner } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

export const ReadOnly = () => <Frame><AccessStateBanner accessState="readOnly" /></Frame>;
export const SettingsLost = () => <Frame><AccessStateBanner accessState="readWrite" settingsLost /></Frame>;
