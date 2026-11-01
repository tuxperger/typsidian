import { SyncIndicator } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const s = (x: any) => ({ overview: null, activity: null, progress: null, error: null, ...x }) as any;
export const Loading = () => <Frame><SyncIndicator sync={s({})} panelOpen={false} onOpenPanel={noop} /></Frame>;
export const Pushing = () => <Frame><SyncIndicator sync={s({ activity: "push" })} panelOpen={false} onOpenPanel={noop} /></Frame>;
export const PullingPanelOpen = () => <Frame><SyncIndicator sync={s({ activity: "pull" })} panelOpen /* панель открыта */ onOpenPanel={noop} /></Frame>;
