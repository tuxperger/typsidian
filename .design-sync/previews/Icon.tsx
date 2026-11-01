import { Icon } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

export const Navigation = () => <Frame><div style={{ display: "flex", gap: 14 }}>{["file","folder","search","network","refresh","chevron","history","sliders"].map((n) => <Icon key={n} name={n as any} size={16} />)}</div></Frame>;
export const Insert = () => <Frame><div style={{ display: "flex", gap: 14 }}>{["heading","list","listnum","sigma","table","code","quote","link","image","board"].map((n) => <Icon key={n} name={n as any} size={16} />)}</div></Frame>;
export const Sizes = () => <Frame><div style={{ display: "flex", gap: 14, alignItems: "center" }}>{[12,14,20,28].map((s) => <Icon key={s} name="file" size={s} />)}</div></Frame>;
