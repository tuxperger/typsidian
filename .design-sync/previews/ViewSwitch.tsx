import { ViewSwitch } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

export const Side = () => <Frame><ViewSwitch value="side" onChange={noop} /></Frame>;
export const Text = () => <Frame><ViewSwitch value="text" onChange={noop} /></Frame>;
export const Document = () => <Frame><ViewSwitch value="document" onChange={noop} /></Frame>;
export const Unavailable = () => <Frame><ViewSwitch value="side" onChange={noop} disabled disabledHint="Окно слишком узкое" /></Frame>;
