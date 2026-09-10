"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { EditorView } from "@codemirror/view";
import type {
  Embed,
  Note,
  ReadError,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { EmbedLink } from "../boards/EmbedLink.js";
import { EmbedStaleBadge } from "../boards/EmbedStaleBadge.js";
import {
  InsertBoardDialog,
  type BoardChoice,
} from "../boards/InsertBoardDialog.js";
import { writeBoardSnapshot } from "../boards/exportSnapshot.js";
import { useNoteEmbeds } from "../boards/useNoteEmbeds.js";
import { CodeEditor, type CaretPosition, type CodeEditorHandle } from "./CodeEditor.js";
import {
  InsertMenu,
  type CaretAnchor,
  type InsertRequest,
} from "./InsertMenu.js";

export interface NoteEditorProps {
  client: VaultClient;
  note: Note | null;
  onClose: () => void;
  /** Доски vault — среди них идёт выбор при вставке (FR-018). */
  boards?: BoardChoice[];
  /**
   * Последнее изменение файла в vault. Приходит пропом, а не второй подпиской
   * на события: `pollEvents` осушает очередь ядра, и второй подписчик
   * перехватывал бы события у первого.
   */
  externalChange?: { path: string; revision: number } | null;
  /** Открыть доску на полотне: переход от вставки к источнику (FR-025). */
  onOpenBoard?: (boardPath: string) => void;
  /** Создать доску, когда вставлять пока нечего (US2 AS2). */
  onCreateBoard?: () => void;
  /**
   * Текущий текст буфера — для предпросмотра (фича 005, FR-002).
   *
   * Именно буфер, а не файл на диске: требование «без кнопки сохранить»
   * (FR-006) иначе невыполнимо. Панель предпросмотра живёт выше по дереву,
   * поэтому текст объявляется наверх, а не читается ею с диска.
   */
  onContentChange?: (text: string) => void;
  /**
   * Доступ к живому редактору — наверх, для порта вставки заготовок
   * (фича 003). Объявляется, а не спрашивается: галерея живёт выше по дереву и
   * до `CodeEditor` не дотягивается.
   *
   * Наверх идёт **функция доступа**, а не сам экземпляр: `CodeEditor`
   * пересоздаётся при смене заметки, и значение, снятое однажды, устарело бы
   * молча. `null` — заметка закрыта: вставлять некуда, и галерея сообщает об
   * этом вместо того, чтобы предлагать неработающую кнопку.
   */
  onEditorReady?: (getView: (() => EditorView | null) | null) => void;
  /** Сохранить открытую заметку как заготовку (фича 003, US2). */
  onSaveAsTemplate?: (content: string) => void;
}

/**
 * Панель просмотра и правки открытой заметки — минимум, без которого нельзя
 * пройти сценарии 2 и 4 `quickstart.md` (заметку нужно открыть, изменить и
 * сохранить).
 *
 * Никаких решений о конфликтах здесь нет: правки объявляются ядру через
 * `beginEdit`, и именно ядро решает, поднимать ли конфликт (FR-021, FR-035).
 * Эта панель только передаёт буфер и вызывает `write` (принцип I).
 */
export function NoteEditor({
  client,
  note,
  onClose,
  boards = [],
  externalChange = null,
  onOpenBoard,
  onCreateBoard,
  onContentChange,
  onEditorReady,
  onSaveAsTemplate,
}: NoteEditorProps): React.JSX.Element {
  const [content, setContent] = useState("");
  const [readError, setReadError] = useState<ReadError | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);
  const [isInserting, setIsInserting] = useState(false);
  const [insertError, setInsertError] = useState<string | null>(null);
  const [isChoosingBoard, setIsChoosingBoard] = useState(false);
  // Вставка идёт в позицию курсора, а её знает только сам textarea.
  // Позиция курсора нужна двум разным потребителям: статус-бару — на отрисовку,
  // вставке доски — в момент нажатия. Первому нужно состояние, второму — всегда
  // свежее значение без ожидания рендера, поэтому и ref, и state.
  const caretRef = useRef<CaretPosition>({ line: 1, column: 1, offset: 0 });
  const [caret, setCaret] = useState<CaretPosition>({ line: 1, column: 1, offset: 0 });
  const onCaretMove = useCallback((next: CaretPosition) => {
    caretRef.current = next;
    setCaret(next);
  }, []);

  // Меню вставки элементов Typst (⌘E). Открытое меню помнит, где была каретка
  // в момент вызова: пока оно висит, курсор в редакторе не двигается, а
  // пересчитывать координаты на каждый кадр незачем.
  const editorRef = useRef<CodeEditorHandle | null>(null);
  const [insertMenu, setInsertMenu] = useState<{ anchor: CaretAnchor | null } | null>(
    null,
  );
  const isInsertMenuOpen = insertMenu !== null;

  const openInsertMenu = useCallback(() => {
    setInsertMenu({ anchor: editorRef.current?.caretCoords() ?? null });
  }, []);

  const closeInsertMenu = useCallback(() => {
    setInsertMenu(null);
    // Работа продолжается в тексте, а не в закрытом меню.
    editorRef.current?.focus();
  }, []);

  const insertElement = useCallback((request: InsertRequest) => {
    setInsertMenu(null);
    // Текст правит редактор: он же вернёт новое содержимое через `onChange`,
    // и отдельного `setContent` здесь быть не должно — иначе буфер обновлялся
    // бы двумя путями сразу.
    editorRef.current?.insertSnippet(request.template, { block: request.block });
  }, []);

  const notePath = note?.path ?? null;
  const embeds = useNoteEmbeds(client, notePath, content);

  // Объявляем буфер наверх одним местом, а не в каждом `setContent`: путей
  // изменения текста четыре (набор, чтение с диска, вставка доски, закрытие
  // заметки), и любой пропущенный означал бы предпросмотр, молча отставший от
  // текста (FR-002).
  useEffect(() => {
    onContentChange?.(content);
  }, [content, onContentChange]);

  // Доступ к редактору объявляется наверх, пока заметка открыта. Функция, а не
  // значение: она спрашивает `editorRef` в момент вставки и потому не может
  // указывать на редактор предыдущей заметки.
  const getView = useCallback(() => editorRef.current?.view() ?? null, []);
  const hasNote = note !== null;
  useEffect(() => {
    onEditorReady?.(hasNote ? getView : null);
    return () => onEditorReady?.(null);
  }, [hasNote, getView, onEditorReady]);

  // ⌘E открывает и закрывает меню вставки, Escape — только закрывает. Так же,
  // как ⌘K у палитры команд: слушатель на окне, потому что открыть меню нужно
  // и тогда, когда фокус не в тексте, а самого меню в дереве ещё нет.
  //
  // `metaKey || ctrlKey` — не различая платформ, как и ⌘K: две раскладки
  // расходились бы там, где приложение обещает одинаковое поведение.
  //
  // Клавиша сверяется и по `code`, и по `key`: интерфейс русский, и при
  // кириллической раскладке `key` для этой клавиши — «у», а не «e». По одному
  // `key` сочетание молча не работало бы ровно у того, для кого написано.
  // Без открытой заметки вставлять некуда, поэтому и слушателя нет.
  useEffect(() => {
    if (!note) return;
    const onKeyDown = (event: KeyboardEvent): void => {
      if (
        (event.metaKey || event.ctrlKey) &&
        (event.code === "KeyE" || event.key.toLowerCase() === "e")
      ) {
        event.preventDefault();
        if (isInsertMenuOpen) closeInsertMenu();
        else openInsertMenu();
      } else if (event.key === "Escape" && isInsertMenuOpen) {
        closeInsertMenu();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [note, isInsertMenuOpen, openInsertMenu, closeInsertMenu]);

  // Изменение файла в vault — повод пересчитать состояние вставок. Затрагивает
  // ли оно эту заметку, решает ядро внутри `fileChanged` (T056).
  const changedPath = externalChange?.path ?? null;
  const changeRevision = externalChange?.revision ?? 0;
  const noticeChange = embeds.fileChanged;
  useEffect(() => {
    if (changedPath !== null) noticeChange(changedPath);
  }, [changedPath, changeRevision, noticeChange]);

  /**
   * Вставка доски (FR-018, FR-019): ядро решает, куда пойдёт снимок и каким
   * текстом описывается вставка; здесь — экспорт движком, запись файла и правка
   * текста заметки в позиции курсора.
   *
   * Порядок важен: снимок пишется до правки текста. Если запись файла не
   * удалась, в заметке не появляется ссылка на несуществующий файл.
   */
  const insertBoard = useCallback(
    async (boardPath: string): Promise<void> => {
      if (notePath === null) return;
      setIsInserting(true);
      setInsertError(null);
      try {
        // Настройки читаются прямо здесь, а не держатся в состоянии
        // редактора: вставка — редкая операция, а политика могла смениться в
        // панели секунду назад. Одно чтение маленького файла дешевле, чем
        // второе место, знающее текущую политику (фича 006, FR-002).
        const overrides = await client.vaultSettings();
        const plan = await client.planEmbed(
          notePath,
          content,
          boardPath,
          overrides,
        );
        const boardContent = await client.readBoard(boardPath);
        const written = await writeBoardSnapshot({
          client,
          plan,
          boardContent,
        });
        if (!written.ok) {
          setInsertError(`Вставить доску не удалось: ${written.reason}.`);
          return;
        }

        const markup = await client.embedMarkup(plan);
        const at = Math.min(caretRef.current.offset, content.length);
        const next = `${content.slice(0, at)}${markup}${content.slice(at)}`;
        setContent(next);
        await client.beginEdit(notePath, next);
        setIsChoosingBoard(false);
      } catch (cause) {
        setInsertError(describeInsertFailure(cause));
      } finally {
        setIsInserting(false);
      }
    },
    [client, notePath, content],
  );

  useEffect(() => {
    let cancelled = false;
    setReadError(null);
    setSaveMessage(null);
    if (!note) {
      setContent("");
      return;
    }
    void (async () => {
      try {
        const text = await client.read(note.path);
        if (!cancelled) setContent(text);
      } catch (cause) {
        if (!cancelled) setReadError(asReadError(cause));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, note]);

  if (!note) {
    return (
      <section className="ts-pane ts-pane--editor" aria-label="Заметка">
        <div className="ts-pane__toolbar">
          <span className="ts-eyebrow">Редактор</span>
        </div>
        <div className="ts-placeholder">
          <p role="status">Выберите заметку в дереве.</p>
        </div>
      </section>
    );
  }

  if (readError) {
    return (
      <section className="ts-pane ts-pane--editor" aria-label="Заметка">
        <div className="ts-pane__toolbar">
          <h2 className="ts-pane__title">{note.name}</h2>
          <span className="ts-spacer" />
          <button type="button" className="ts-btn" onClick={onClose}>
            Закрыть
          </button>
        </div>
        <div className="ts-placeholder">
          <p role="alert">{describeReadError(readError)}</p>
        </div>
      </section>
    );
  }

  return (
    <section className="ts-pane ts-pane--editor" aria-label="Заметка">
      <div className="ts-pane__toolbar">
        <h2 className="ts-pane__title">{note.name}</h2>
        <span className="ts-spacer" />
        <div role="group" aria-label="Действия с заметкой">
          {/* Меню вставки открывается по ⌘E; кнопка здесь для того, кто про
              сочетание не знает, — сама она ничего не вставляет.
              Только открывает, не переключает: открытое меню закрывается от
              нажатия мимо себя, и нажатие на эту кнопку для него тоже «мимо» —
              переключатель успевал бы закрыться до собственного onClick. */}
          <button
            type="button"
            className="ts-btn"
            title="Вставить элемент Typst · ⌘E"
            aria-haspopup="dialog"
            aria-expanded={isInsertMenuOpen}
            onClick={openInsertMenu}
          >
            Вставить элемент
          </button>
          {/* SC-007: от открытой заметки до вставленной доски — два действия,
              эта кнопка и выбор доски в списке. */}
          <button
            type="button"
            className="ts-btn"
            onClick={() => {
              setInsertError(null);
              setIsChoosingBoard(true);
            }}
          >
            Вставить доску
          </button>
          {/* SC-006: от открытой заметки до сохранённой заготовки — три
              действия, эта кнопка, ввод имени и подтверждение. Сохраняется
              **буфер**, а не файл с диска: заготовка фиксирует то, что
              пользователь видит перед собой (FR-027). */}
          {onSaveAsTemplate && (
            <button
              type="button"
              className="ts-btn"
              onClick={() => onSaveAsTemplate(content)}
            >
              Сохранить как заготовку
            </button>
          )}
          <button
            type="button"
            className="ts-btn ts-btn--accent"
            onClick={() => {
              void (async () => {
                try {
                  await client.write(note.path, content, undefined);
                  setSaveMessage("Сохранено.");
                } catch {
                  setSaveMessage("Сохранить не удалось — правки не потеряны.");
                }
              })();
            }}
          >
            Сохранить
          </button>
          <button type="button" className="ts-btn" onClick={onClose}>
            Закрыть
          </button>
        </div>
      </div>
      <CodeEditor
        ref={editorRef}
        ariaLabel={`Содержимое заметки ${note.name}`}
        value={content}
        onCaretMove={onCaretMove}
        onChange={(next) => {
          setContent(next);
          // FR-035: ядро должно знать о несохранённых правках и держать сам
          // буфер — иначе внешнее изменение будет применено молча.
          void client.beginEdit(note.path, next);
        }}
      />

      {insertMenu && (
        <InsertMenu
          anchor={insertMenu.anchor}
          onInsert={insertElement}
          onClose={closeInsertMenu}
        />
      )}

      {/* Вставки заметки и их актуальность (FR-027, FR-031, FR-032). Список, а
          не пометки внутри текста: индикатор рядом с вызовом требует редактора,
          знающего позиции в тексте, — в textarea их не разместить, а врать о
          месте хуже, чем показать список. */}
      {embeds.embeds.length > 0 && (
        <ul className="ts-embeds" aria-label="Вставленные доски">
          {embeds.embeds.map((embed) => (
            <li key={embed.id.value} className="ts-embeds__item">
              {onOpenBoard && <EmbedLink embed={embed} onOpenBoard={onOpenBoard} />}
              <EmbedStaleBadge
                embed={embed}
                isUpdating={embeds.isUpdating}
                error={embeds.error}
                onUpdate={(target) => void embeds.update(target)}
                onRestore={(target) => void embeds.update(target)}
              />
            </li>
          ))}
        </ul>
      )}

      <InsertBoardDialog
        isOpen={isChoosingBoard}
        boards={boards}
        isInserting={isInserting}
        error={insertError}
        onInsert={(boardPath) => void insertBoard(boardPath)}
        {...(onCreateBoard ? { onCreateBoard } : {})}
        onCancel={() => setIsChoosingBoard(false)}
      />
      {/* Статус-бар макета (handoff § Editor pane). Показывает только то, что
          известно на самом деле: пилюля vim-режима здесь надпись, а не
          переключатель, — моста к Neovim в web-сборке не существует
          (research.md §7), и кликабельная пилюля утверждала бы, что режим
          переключается. */}
      <div className="ts-statusbar">
        <span className="ts-vim" title="Режим редактора: моста к Neovim в web-сборке нет">
          NORMAL
        </span>
        <span>
          Ln {caret.line}, Col {caret.column}
        </span>
        <span>UTF-8</span>
        {saveMessage && <p role="status">{saveMessage}</p>}
        <span className="ts-spacer" />
        <span>{countWords(content)}</span>
        <span className="ts-tag">typst</span>
      </div>
    </section>
  );
}

/**
 * Счётчик слов для статус-бара («142 слова» в макете).
 *
 * Считает по пробелам, вместе с разметкой Typst: отделять команды от текста
 * значило бы разбирать язык, а разбор живёт в компиляторе, не здесь.
 */
function countWords(text: string): string {
  const count = text.split(/\s+/).filter((word) => word.length > 0).length;
  const mod100 = count % 100;
  const word =
    mod100 >= 11 && mod100 <= 14
      ? "слов"
      : count % 10 === 1
        ? "слово"
        : count % 10 >= 2 && count % 10 <= 4
          ? "слова"
          : "слов";
  return `${count} ${word}`;
}

/** Почему вставка не состоялась (FR-018, FR-019). */
function describeInsertFailure(cause: unknown): string {
  const kind =
    typeof cause === "object" && cause !== null && "kind" in cause
      ? (cause as { kind: unknown }).kind
      : null;
  switch (kind) {
    case "NotANote":
      return "Вставлять доску можно только в текстовую заметку.";
    case "NotABoard":
      return "Вставить можно только доску.";
    case "NotFound":
      return "Доска не найдена — возможно, её удалили или переименовали снаружи.";
    case "Unreadable":
      return "Файл доски не читается: он повреждён или создан несовместимой версией.";
    default:
      return "Вставить доску не удалось.";
  }
}

/**
 * Причина отказа чтения называется словами (FR-008, FR-014). `TooLarge` —
 * отдельный случай: заметка существует и видна в дереве, отказано именно в
 * открытии, и пользователю важно понимать разницу.
 */
function describeReadError(error: ReadError): string {
  switch (error.kind) {
    case "NotFound":
      return "Заметка не найдена — возможно, её удалили или переименовали снаружи.";
    case "TooLarge":
      return "Заметка больше 10 МБ — это выше поддерживаемого размера, и открыть её нельзя. Файл на диске не тронут.";
    case "Io":
      return "Не удалось прочитать заметку с диска.";
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}

/** Сужение `unknown` до `ReadError` без `any` (принцип IV). */
function asReadError(cause: unknown): ReadError {
  if (typeof cause === "object" && cause !== null && "kind" in cause) {
    const { kind } = cause as { kind: unknown };
    if (kind === "NotFound" || kind === "TooLarge" || kind === "Io") {
      return { kind } as ReadError;
    }
  }
  return { kind: "Io" };
}
