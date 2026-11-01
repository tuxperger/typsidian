import { InsertBoardDialog } from "@typsidian/ui";
import { IconSprite } from "@typsidian/ui";
const Frame = ({ children, h }: any) => (
  <div style={{ background: "var(--steel-950)", color: "var(--steel-200)", padding: 16, minHeight: h, position: "relative", fontFamily: "var(--font-sans)" }}>
    <IconSprite />
    {children}
  </div>
);
const noop = () => {};

const boards: any = [{ path: "Схема установки.excalidraw", name: "Схема установки" }, { path: "Граф эксперимента.excalidraw", name: "Граф эксперимента" }];
export const WithBoards = () => <Frame h="100vh"><InsertBoardDialog isOpen boards={boards} onInsert={noop} onCancel={noop} /></Frame>;
export const Empty = () => <Frame h="100vh"><InsertBoardDialog isOpen boards={[]} onInsert={noop} onCreateBoard={noop} onCancel={noop} /></Frame>;
export const Failed = () => <Frame h="100vh"><InsertBoardDialog isOpen boards={boards} error="Не удалось записать снимок доски." onInsert={noop} onCancel={noop} /></Frame>;
