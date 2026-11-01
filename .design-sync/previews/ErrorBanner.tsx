import { ErrorBanner } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

export const InvalidName = () => <Frame><ErrorBanner error={{ operation: "rename", error: { kind: "InvalidName" } } as any} onDismiss={noop} /></Frame>;
export const AccessDenied = () => <Frame><ErrorBanner error={{ operation: "delete", error: { kind: "AccessDenied" } } as any} onDismiss={noop} /></Frame>;
export const NotFound = () => <Frame><ErrorBanner error={{ operation: "delete", error: { kind: "NotFound" } } as any} /></Frame>;
