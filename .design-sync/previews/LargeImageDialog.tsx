import { LargeImageDialog } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

export const Large = () => <Frame h="100vh"><LargeImageDialog request={{ name: "thermogram.png", size: 24117248, folder: "Результаты/data" }} onAnswer={noop} /></Frame>;
