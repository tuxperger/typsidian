"use client";

import { useEffect, useRef } from "react";
import type { VaultClient } from "../vault-client.js";
import { BoardCanvas } from "./BoardCanvas.js";
import type { BoardControllers } from "./boardControllers.js";
import { useBoardDocument } from "./useBoardDocument.js";

// Доска во вкладке (фича 014, research.md §8). Раньше документ доски жил в
// `Workspace.tsx` — одна открытая доска на окно; теперь он принадлежит вкладке:
// хук документа, регистрация «дописать по требованию» и предупреждение перед
// закрытием окна переехали сюда вместе с полотном.
//
// Уход с вкладки дописывает доску: размонтирование сбрасывает дебаунс, и без
// этого последние штрихи остались бы в буфере (research.md §6 фичи 002).

export interface BoardTabProps {
  client: VaultClient;
  path: string;
  /** Имя файла доски — заголовок панели полотна. */
  name: string;
  controllers: BoardControllers;
}

export function BoardTab({ client, path, name, controllers }: BoardTabProps): React.JSX.Element {
  const board = useBoardDocument(client, path);
  const { flush, replace } = board;

  // Снимок истории делается только с записанного (011 FR-006): клиент обязан
  // уметь дописать доску по требованию, не дожидаясь дебаунса.
  useEffect(() => client.registerFlusher(async () => void (await flush())), [client, flush]);

  // Реестр узнаёт, как дописать эту доску при закрытии вкладки.
  useEffect(
    () =>
      controllers.register(path, {
        flush: async () => (await flush()).state === "Clean",
        replace,
      }),
    [controllers, path, flush, replace],
  );

  // Уход с вкладки (и размонтирование при смене вида): дописать и запомнить отказ.
  const flushRef = useRef(flush);
  flushRef.current = flush;
  useEffect(
    () => () => {
      void flushRef.current().then((state) => {
        if (state.state !== "Clean") controllers.noteFailure(path);
      });
    },
    [controllers, path],
  );

  // Несохранённое на полотне удерживает окно так же, как несохранённый текст
  // (FR-011 фичи 002).
  useEffect(() => {
    if (board.saveState.state === "Clean") return;
    const warn = (event: BeforeUnloadEvent): void => {
      event.preventDefault();
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [board.saveState]);

  return (
    <BoardCanvas
      name={name}
      content={board.content}
      contentRevision={board.contentRevision}
      saveState={board.saveState}
      readError={board.readError}
      // FR-034: клиент без права записи доски объявляет это пользователю и не
      // даёт инструментов. Само право приходит из ядра через `VaultClient`, а не
      // выводится здесь из таргета: ветвления по платформе в общем UI быть не
      // должно (принцип II).
      viewModeEnabled={client.boardWriteProfile === "ViewOnly"}
      onChange={board.changed}
    />
  );
}
