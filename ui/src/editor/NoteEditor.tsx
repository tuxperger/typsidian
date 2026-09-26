"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { EditorView } from "@codemirror/view";
import type {
  ElementSpec,
  Embed,
  Note,
  ReadError,
  TextEdit,
} from "../wasm-core/typsidian_core.js";
import type { VaultClient } from "../vault-client.js";
import { hintWith, type ShortcutsState } from "../workspace/useShortcuts.js";
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
import { useNoteAutoSave, type NoteSaveState } from "./useNoteAutoSave.js";
import { InsertPanel, useInsertPanelVisible } from "./InsertPanel.js";
import { elementById, type TypstElement } from "./typstElements.js";
import { ParamForm, type ParamFormMode } from "./ParamForm.js";
import {
  describeUnavailable,
  NOTHING_TO_EDIT,
  useElementAtCaret,
} from "./useElementAtCaret.js";
import type { SnippetFieldState } from "./snippet.js";
import type { EditorFontSizeControl } from "../settings/useEditorFontSizeControl.js";
import { useContextMenu } from "../menu/ContextMenu.js";
import { editorMenuItems, placeCaretAt } from "./editorContextMenu.js";

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
  /**
   * Версия заметки, принятая при разрешении конфликта (FR-022, FR-023).
   *
   * Ядро отдаёт её тому, кто решение исполнил, со словами «редактор обязан
   * показать это вместо буфера» (`resolve_conflict_accept_external`), а решение
   * принимает диалог выше по дереву — отсюда проп.
   *
   * С автосохранением это перестало быть украшением: буфер, оставленный после
   * «принять внешнюю», ушёл бы в файл сам и отменил только что сделанный выбор
   * пользователя.
   */
  acceptedExternal?: { path: string; text: string; revision: number } | null;
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
  /**
   * Размер шрифта редактора, меняемый на месте: колесом с Ctrl/⌘ и ступенями в
   * статус-баре (004 FR-016, FR-017).
   *
   * Приходит пропом, а не создаётся здесь: значение живёт в настройках открытой
   * папки, а ими владеет рабочая область. Не передан — жестов нет, и размер
   * меняется только панелью настроек, как раньше.
   */
  fontSize?: EditorFontSizeControl;
  /**
   * Действующие сочетания клавиш. Пропом, а не своим разбором: один разбор на
   * приложение, и подпись в меню вставки обязана совпадать с тем, что работает.
   */
  shortcuts: ShortcutsState;
  /**
   * Счётчик запросов «Параметры элемента» извне — из палитры команд (фича 008,
   * FR-006). Меняется — форма открывается для элемента под кареткой.
   */
  paramsRequest?: number;
}

/**
 * Панель просмотра и правки открытой заметки — минимум, без которого нельзя
 * пройти сценарии 2 и 4 `quickstart.md` (заметку нужно открыть и изменить).
 *
 * Сохранять её отдельным действием больше не нужно: набранное уходит в файл само
 * (`useNoteAutoSave`), как это давно делают доски, и кнопки «Сохранить» здесь
 * нет. Состояние записи видно в статус-баре — иначе автосохранение было бы
 * обещанием без подтверждения.
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
  acceptedExternal = null,
  onOpenBoard,
  onCreateBoard,
  onContentChange,
  onEditorReady,
  onSaveAsTemplate,
  fontSize,
  shortcuts,
  paramsRequest = 0,
}: NoteEditorProps): React.JSX.Element {
  const [content, setContent] = useState("");
  const [readError, setReadError] = useState<ReadError | null>(null);
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
  // Смещение косой черты, которой вызвали меню (макет § Меню вставки).
  // `null` — меню открыли сочетанием клавиш, и заменять в тексте нечего.
  // В ref, а не в состоянии: значение нужно в момент вставки, а не на отрисовку.
  const slashAt = useRef<number | null>(null);
  // Форма параметров элемента (фича 008): `null` — закрыта. Схему и режим
  // (вставка или правка найденного под кареткой) отдаёт ядро.
  const [paramForm, setParamForm] = useState<{
    spec: ElementSpec;
    title: string;
    mode: ParamFormMode;
    block: boolean;
  } | null>(null);
  // Где курсор среди полей вставленного сниппета — подсказка в статус-баре.
  const [fieldState, setFieldState] = useState<SnippetFieldState | null>(null);
  const contextMenu = useContextMenu();

  const openInsertMenu = useCallback(() => {
    slashAt.current = null;
    setInsertMenu({ anchor: editorRef.current?.caretCoords() ?? null });
  }, []);

  // Вызов по `/`: меню встаёт у той же каретки, а сама косая черта остаётся в
  // тексте — её уберёт вставка. Отказ от выбора (Escape, нажатие мимо) её не
  // трогает: набранное пользователем приложение не стирает.
  const openInsertMenuAt = useCallback((at: number) => {
    slashAt.current = at;
    setInsertMenu({ anchor: editorRef.current?.caretCoords() ?? null });
  }, []);

  const closeInsertMenu = useCallback(() => {
    setInsertMenu(null);
    slashAt.current = null;
    // Работа продолжается в тексте, а не в закрытом меню.
    editorRef.current?.focus();
  }, []);

  /**
   * Вставляет шаблон в редактор.
   *
   * `range` — что заменить: диапазон правимого блока. Не передан — снимается
   * косая черта вызова, если меню открыли ею, а иначе вставка идёт в позицию
   * курсора.
   */
  const applyTemplate = useCallback(
    (template: string, block: boolean, range?: { from: number; to: number }) => {
      const from = slashAt.current;
      slashAt.current = null;
      editorRef.current?.insertSnippet(template, {
        block,
        ...(range === undefined
          ? from === null
            ? {}
            : { replaceFrom: from }
          : { replaceFrom: range.from, replaceTo: range.to }),
      });
    },
    [],
  );

  /**
   * Вставка элемента из меню или с панели. Элемент, у которого ядро велит
   * открыть форму (contracts § «Когда форма»), идёт через неё; остальные —
   * сниппетом, как раньше (FR-003).
   */
  const insertElement = useCallback(
    async (request: InsertRequest) => {
      setInsertMenu(null);
      const spec = request.id ? await client.elementSchema(request.id) : undefined;
      if (spec?.form_on_insert && request.id) {
        setParamForm({
          spec,
          title: elementById(request.id)?.name ?? spec.typst,
          mode: { kind: "insert" },
          block: request.block,
        });
        return;
      }
      // Текст правит редактор: он же вернёт новое содержимое через `onChange`,
      // и отдельного `setContent` здесь быть не должно — иначе буфер обновлялся
      // бы двумя путями сразу.
      applyTemplate(request.template, request.block);
    },
    [applyTemplate, client],
  );

  /** Текст заметки сейчас — форме, чтобы ядро сверяло элемент и метки. */
  const noteText = useCallback(
    () => editorRef.current?.view()?.state.doc.toString() ?? content,
    [content],
  );

  // Элемент под кареткой — для кнопки «Параметры» на панели (T033, FR-006).
  const caretElement = useElementAtCaret(client, content, caret.offset);

  /**
   * «Параметры» элемента под кареткой (или в `offset`). Где правка недоступна,
   * причина называется словами (FR-011, US2 AS5), а не молчанием.
   */
  const openParams = useCallback(
    async (offset?: number) => {
      const view = editorRef.current?.view();
      const text = view?.state.doc.toString() ?? content;
      const at = offset ?? view?.state.selection.main.head ?? caretRef.current.offset;
      const found = await client.elementAt(text, at);
      if (found.kind === "None") {
        contextMenu.notify(NOTHING_TO_EDIT);
        return;
      }
      if (found.kind === "Unavailable") {
        contextMenu.notify(describeUnavailable(found.reason));
        return;
      }
      const spec = await client.elementSchema(found.element.id);
      if (!spec) return;
      setInsertMenu(null);
      setParamForm({
        spec,
        title: elementById(found.element.id)?.name ?? spec.typst,
        mode: { kind: "edit", parsed: found.element },
        block: false,
      });
    },
    [client, content, contextMenu],
  );

  const closeForm = useCallback(() => {
    setParamForm(null);
    slashAt.current = null;
    editorRef.current?.focus();
  }, []);

  /**
   * Вставить собранный формой текст. Сырым текстом, а не сниппетом: в тексте
   * Typst `${…}` — обычные символы формулы, а не поля сниппета.
   */
  const insertRendered = useCallback((text: string, block: boolean) => {
    const view = editorRef.current?.view();
    if (!view) return;
    const range = view.state.selection.main;
    const from = slashAt.current ?? range.from;
    const to = slashAt.current === null ? range.to : Math.max(range.to, from + 1);
    slashAt.current = null;
    // Блочный элемент начинает строку: `#figure` посреди абзаца разорвал бы его.
    const lineStart = view.state.doc.lineAt(from).from;
    const needsBreak = block && view.state.sliceDoc(lineStart, from).trim() !== "";
    const insert = needsBreak ? `\n${text}` : text;
    view.dispatch({
      changes: { from, to, insert },
      selection: { anchor: from + insert.length },
      userEvent: "input",
      scrollIntoView: true,
    });
    setParamForm(null);
    view.focus();
  }, []);

  /** Применить правку: все замены — одной транзакцией, одним шагом отмены (FR-010). */
  const applyEdits = useCallback((edits: TextEdit[]) => {
    const view = editorRef.current?.view();
    if (!view) return;
    view.dispatch({
      changes: edits.map((edit) => ({ from: edit.from, to: edit.to, insert: edit.insert })),
      userEvent: "input",
    });
    setParamForm(null);
    view.focus();
  }, []);

  // Запрос из палитры команд (FR-006): счётчик сменился — открыть «Параметры».
  const lastParamsRequest = useRef(paramsRequest);
  useEffect(() => {
    if (paramsRequest === lastParamsRequest.current) return;
    lastParamsRequest.current = paramsRequest;
    void openParams();
  }, [paramsRequest, openParams]);

  // Панель вставки (макет § Панель вставки). Показана по умолчанию и убирается
  // кнопкой в тулбаре заметки: она для мыши и меню не заменяет.
  const [panelVisible, togglePanel] = useInsertPanelVisible();

  /**
   * Вставка с панели идёт тем же путём, что из меню: элемент с параметрами
   * открывает диалог, остальные вставляются сниппетом. Второй дороги к тексту
   * здесь не заводится.
   */
  const insertFromPanel = useCallback(
    (element: TypstElement) => {
      void insertElement({
        id: element.id,
        template: element.template,
        block: element.block ?? false,
      });
    },
    [insertElement],
  );

  const notePath = note?.path ?? null;
  const embeds = useNoteEmbeds(client, notePath, content);

  // Автосохранение вместо кнопки (FR-006 фичи 005 — предпросмотр собирается из
  // буфера, а буфер теперь сам доходит до диска).
  const autoSave = useNoteAutoSave(client, notePath);
  const flushSave = autoSave.flush;

  // Переключение на другую заметку и размонтирование обязаны дописать текущую:
  // иначе последние нажатия остались бы в буфере дебаунса. `flushSave` меняется
  // вместе с путём, поэтому уборка достаётся той заметке, которая закрывается, а
  // не той, что открылась.
  useEffect(() => () => void flushSave(), [flushSave]);

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
  // Сверка по `code` и `key` вместе никуда не делась — она переехала в
  // `useShortcuts`, а имя физической клавиши считает ядро: интерфейс русский, и
  // при кириллической раскладке `key` для этой клавиши — «у», а не «e».
  // Без открытой заметки вставлять некуда, поэтому и слушателя нет.
  useEffect(() => {
    if (!note) return;
    const onKeyDown = (event: KeyboardEvent): void => {
      if (shortcuts.matches("insert-menu", event)) {
        event.preventDefault();
        if (isInsertMenuOpen) closeInsertMenu();
        else openInsertMenu();
      }
      // «Параметры элемента» под кареткой (фича 008, FR-006) — пара к меню
      // вставки. Пока форма открыта, повторное нажатие её не перебивает.
      if (shortcuts.matches("element-params", event) && paramForm === null) {
        event.preventDefault();
        void openParams();
      }
      // Escape отсюда ушёл: меню закрывает себя само через
      // `useDismissOnEscape`, и поверх него может стоять диалог параметров —
      // закрыться обязан он один.
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [note, isInsertMenuOpen, openInsertMenu, closeInsertMenu, shortcuts, paramForm, openParams]);

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
        // Вставка — такая же правка буфера, как набор текста, и уходит в файл
        // тем же путём: иначе разметка вставки жила бы только в окне.
        autoSave.changed(next);
        setIsChoosingBoard(false);
      } catch (cause) {
        setInsertError(describeInsertFailure(cause));
      } finally {
        setIsInserting(false);
      }
    },
    [client, notePath, content, autoSave],
  );

  const acceptSaved = autoSave.accept;
  useEffect(() => {
    let cancelled = false;
    setReadError(null);
    if (!note) {
      setContent("");
      return;
    }
    void (async () => {
      try {
        const text = await client.read(note.path);
        if (cancelled) return;
        setContent(text);
        // Прочитанное с диска — не правка: автосохранению говорится, что именно
        // этот текст сейчас в файле, иначе первое же открытие заметки
        // перезаписывало бы её тем же содержимым.
        acceptSaved(text);
      } catch (cause) {
        if (!cancelled) setReadError(asReadError(cause));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client, note, acceptSaved]);

  // Принятая внешняя версия заменяет буфер (FR-022). Записывать её обратно не
  // нужно и нельзя: она уже на диске — ядро её оттуда и прочитало, — поэтому
  // автосохранению она объявляется как сохранённая, а не как правка.
  const acceptedPath = acceptedExternal?.path ?? null;
  const acceptedText = acceptedExternal?.text ?? null;
  const acceptedRevision = acceptedExternal?.revision ?? 0;
  useEffect(() => {
    if (acceptedText === null || acceptedPath === null || acceptedPath !== notePath) {
      return;
    }
    setContent(acceptedText);
    acceptSaved(acceptedText);
    // `acceptedRevision` в зависимостях намеренно: он и есть повод применить
    // версию заново, если пользователь принял внешнюю дважды подряд.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [acceptedPath, acceptedText, acceptedRevision, notePath, acceptSaved]);

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
          {/* Пока панель вставки показана, эта кнопка не нужна: там та же
              дверь в каталог («Все элементы»), и две одинаковые в соседних
              строках только теснят тулбар. */}
          {!panelVisible && (
            <button
              type="button"
              className="ts-btn"
              title={hintWith(
                "Вставить элемент Typst",
                shortcuts.displayOf("insert-menu"),
              )}
              aria-haspopup="dialog"
              aria-expanded={isInsertMenuOpen}
              onClick={openInsertMenu}
            >
              Вставить элемент
            </button>
          )}
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
          {/* Выключатель панели вставки. В тулбаре, а не в самой панели:
              убранную панель нечем было бы вернуть. */}
          <button
            type="button"
            className="ts-btn"
            aria-pressed={panelVisible}
            title="Панель вставки — строка кнопок над текстом"
            onClick={togglePanel}
          >
            Панель
          </button>
          {/* Кнопки «Сохранить» здесь нет намеренно: набранное уходит в файл
              само, а состояние записи видно в статус-баре. Оставить её значило
              бы завести второй путь к тому же `write` и второй ответ на вопрос
              «сохранено ли». */}
          <button type="button" className="ts-btn" onClick={onClose}>
            Закрыть
          </button>
        </div>
      </div>
      {panelVisible && (
        <InsertPanel
          shortcut={shortcuts.displayOf("insert-menu")}
          onInsert={insertFromPanel}
          onOpenMenu={openInsertMenu}
          // «Параметры» элемента под кареткой (FR-006): кнопка видна, когда
          // каретка в элементе схемы; недоступная — с причиной (FR-011).
          params={
            caretElement?.kind === "Found"
              ? { available: true }
              : caretElement?.kind === "Unavailable"
                ? { available: false, reason: describeUnavailable(caretElement.reason) }
                : null
          }
          paramsShortcut={shortcuts.displayOf("element-params")}
          onParams={() => void openParams()}
        />
      )}

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
          autoSave.changed(next);
        }}
        onSlashMenu={openInsertMenuAt}
        onContextMenu={(event) => {
          const view = editorRef.current?.view();
          if (!view || event.shiftKey) return;
          placeCaretAt(view, event.clientX, event.clientY);
          contextMenu.open(
            event,
            editorMenuItems(view, {
              openInsertMenu,
              // Позиция — свежая, после `placeCaretAt`: состояние `caret`
              // догонит клик только на следующей отрисовке.
              openParams: () => void openParams(view.state.selection.main.head),
              paramsShortcut: shortcuts.displayOf("element-params"),
              chooseBoard: () => {
                setInsertError(null);
                setIsChoosingBoard(true);
              },
              saveAsTemplate: onSaveAsTemplate
                ? () => onSaveAsTemplate(view.state.doc.toString())
                : undefined,
              notify: contextMenu.notify,
              insertShortcut: shortcuts.displayOf("insert-menu"),
            }),
          );
        }}
        onFieldState={setFieldState}
        {...(fontSize ? { onZoom: fontSize.step } : {})}
      />

      {insertMenu && (
        <InsertMenu
          anchor={insertMenu.anchor}
          onInsert={(request) => void insertElement(request)}
          onClose={closeInsertMenu}
        />
      )}

      {/* Форма параметров элемента (фича 008; макет «Вставка» § Параметры
          блока). Поля строятся по схеме из ядра; занятые метки, текст
          «Будет вставлено» и план правки тоже считает ядро. */}
      {paramForm !== null && (
        <ParamForm
          key={paramForm.mode.kind === "edit" ? paramForm.mode.parsed.revision : paramForm.spec.id}
          client={client}
          spec={paramForm.spec}
          title={paramForm.title}
          mode={paramForm.mode}
          noteText={noteText}
          onInsert={(text) => insertRendered(text, paramForm.block)}
          onApply={applyEdits}
          onCancel={closeForm}
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
        {/* Состояние полей вставленного сниппета (макет «Вставка» § состояние
            полей): без счётчика подсветка говорит только «что-то выделено». */}
        {fieldState && (
          <span className="ts-fieldstate">
            поле {fieldState.index} из {fieldState.count} · ⇥ следующее · esc выйти
          </span>
        )}
        <SaveIndicator state={autoSave.state} />
        <span className="ts-spacer" />

        {/* Кегль редактора — те же ступени, что у масштаба предпросмотра, и
            тот же жест Ctrl/⌘ + колесо поверх текста. Значение — сама кнопка:
            нажатие снимает перекрытие папки и возвращает размер устройства. */}
        {fontSize && (
          <div className="ts-zoom" role="group" aria-label="Размер шрифта редактора">
            <button
              type="button"
              className="ts-btn ts-zoom__step"
              onClick={() => fontSize.step(-1)}
              aria-label="Уменьшить шрифт редактора"
              title="Уменьшить шрифт (Ctrl + колесо)"
            >
              −
            </button>
            <button
              type="button"
              className="ts-btn ts-zoom__value"
              onClick={fontSize.reset}
              disabled={!fontSize.isOverridden}
              aria-label={`Шрифт ${fontSize.size} пикселей, вернуть размер по умолчанию`}
              title="Вернуть размер по умолчанию"
            >
              {fontSize.size}&nbsp;px
            </button>
            <button
              type="button"
              className="ts-btn ts-zoom__step"
              onClick={() => fontSize.step(1)}
              aria-label="Увеличить шрифт редактора"
              title="Увеличить шрифт (Ctrl + колесо)"
            >
              +
            </button>
          </div>
        )}

        <span>{countWords(content)}</span>
        <span className="ts-tag">typst</span>
      </div>
    </section>
  );
}

/**
 * Состояние автосохранения в статус-баре.
 *
 * Показывается всегда, в том числе «сохранено»: без кнопки это единственное
 * место, по которому видно, что набранное дошло до диска.
 *
 * Живая область — только у отказа. «Не сохранено → сохранение… → сохранено»
 * сменяется на каждой паузе в наборе, и объявляй это программа чтения с экрана,
 * она перебивала бы сам набор текста. Отказ — другое дело: о нём пользователь
 * обязан узнать сразу, а не при следующем взгляде на строку.
 */
function SaveIndicator({ state }: { state: NoteSaveState }): React.JSX.Element {
  if (state.state === "Failed") {
    return (
      <p className="ts-save ts-save--failed" role="alert">
        {state.reason}
      </p>
    );
  }
  const label =
    state.state === "Saving"
      ? "сохранение…"
      : state.state === "Dirty"
        ? "не сохранено"
        : state.state === "Held"
          ? "конфликт — сохранение приостановлено"
          : "сохранено";
  return <p className="ts-save">{label}</p>;
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
