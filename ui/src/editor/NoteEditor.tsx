"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { EditorView } from "@codemirror/view";
import type {
  Condition,
  ElementSpec,
  LibraryStatus,
  ReadError,
  TabId,
  TextEdit,
  TextPlace,
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
import type { NoteDocument, NoteSaveState } from "./noteDocument.js";
import { useDocumentSnapshot } from "./useNoteDocument.js";
import { InsertPanel, useInsertPanelVisible } from "./InsertPanel.js";
import { elementById, type LocalizedElement } from "./typstElements.js";
import { ParamForm, SELECT_ELEMENT_ID, type ParamFormMode } from "./ParamForm.js";
import {
  describeUnavailable,
  useElementAtCaret,
} from "./useElementAtCaret.js";
import { parseSnippet, type SnippetFieldState } from "./snippet.js";
import type { EditorFontSizeControl } from "../settings/useEditorFontSizeControl.js";
import { useContextMenu } from "../menu/ContextMenu.js";
import { editorMenuItems, placeCaretAt } from "./editorContextMenu.js";
import { typstHighlightFor } from "./highlight/typstHighlight.js";
import { useI18n, type Phrase, type Translate } from "../i18n/index.js";

export interface NoteEditorProps {
  client: VaultClient;
  /**
   * Заметка Typst — по виду файла, который определило ядро (`FileKind::Note`),
   * а не по расширению здесь (принцип I). Только ей нужна подсветка Typst
   * (фича 024, FR-012).
   */
  typst?: boolean;
  /**
   * Документ на путь (фича 014): текст, чтение с диска и автосохранение
   * принадлежат ему, а не этому компоненту. Заметка в двух группах — один
   * документ и два редактора.
   */
  document: NoteDocument;
  /** Имя файла с расширением — для подписи поля и предложения имени заготовки. */
  name: string;
  /** Вкладка, которой принадлежит редактор: по ней он поднимается из снимка. */
  tabId: TabId;
  /**
   * Вкладка активна в группе с кареткой. Слушатели окна (меню вставки,
   * параметры элемента), доступ к редактору для порта вставки и запрос из
   * палитры работают только у неё — иначе при двух группах команда
   * срабатывала бы дважды.
   */
  active: boolean;
  /** Куда поставить каретку после перехода («Назад», ссылка, ошибка); `null` — никуда. */
  place?: TextPlace | null;
  /** Место исполнено: хук вкладок забывает его. */
  onPlaceShown?: () => void;
  /** Растёт, когда фокус надо отдать редактору (щелчок по вкладке). */
  focusRequest?: number;
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
   * Доступ к живому редактору — наверх, для порта вставки заготовок
   * (фича 003). Объявляется, а не спрашивается: галерея живёт выше по дереву и
   * до `CodeEditor` не дотягивается.
   *
   * Наверх идёт **функция доступа**, а не сам экземпляр: `CodeEditor`
   * пересоздаётся при смене вкладки, и значение, снятое однажды, устарело бы
   * молча. `null` — вкладка перестала быть активной: вставлять некуда, и галерея
   * сообщает об этом вместо того, чтобы предлагать неработающую кнопку.
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
  /**
   * Запрос «вставить выборку» извне - из поиска по Mod+Enter (фича 020, B3):
   * новый `id` открывает форму блока с этими условиями.
   */
  selectRequest?: { id: number; conditions: Condition[] } | null;
  /** Итог записи библиотеки выборки: `Customized` - предупреждение в колонке свойств. */
  onLibraryStatus?: (status: LibraryStatus) => void;
  /**
   * Связи открытой заметки одной строкой для статус-бара («5 ссылок · 2
   * упоминания»). Не передан — сегмента нет: он показывается только при
   * показанной колонке заметки.
   */
  linksSummary?: string;
  /**
   * Сводка вкладок для статус-бара («6 вкладок», «группа 1 из 2 · 4 вкладки»).
   * Передаётся редактору активной группы: статус-бар в коде у каждого редактора
   * свой, а в макете один на окно (research.md §14).
   */
  tabsSummary?: string;
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
  typst = false,
  document,
  name,
  tabId,
  active,
  place = null,
  onPlaceShown,
  focusRequest = 0,
  boards = [],
  externalChange = null,
  onOpenBoard,
  onCreateBoard,
  onEditorReady,
  onSaveAsTemplate,
  fontSize,
  shortcuts,
  paramsRequest = 0,
  selectRequest = null,
  onLibraryStatus,
  linksSummary,
  tabsSummary,
}: NoteEditorProps): React.JSX.Element {
  const { t } = useI18n();
  const snapshot = useDocumentSnapshot(document);
  const content = snapshot.text;
  const [isInserting, setIsInserting] = useState(false);
  const [insertError, setInsertError] = useState<Phrase | null>(null);
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
   * Форма блока выборки (фича 020, B4). Схему отдаёт ядро по ключу
   * `call:select`; условия поиска, если есть, становятся начальным значением
   * поля «условия».
   */
  const openSelectForm = useCallback(
    async (conditions?: Condition[]) => {
      const spec = await client.elementSchema(SELECT_ELEMENT_ID);
      if (!spec) return;
      setInsertMenu(null);
      const initial =
        conditions !== undefined && conditions.length > 0
          ? [
              {
                name: "where",
                source: "",
                plain: JSON.stringify(conditions),
                expression: false,
                from: 0,
                to: 0,
              },
            ]
          : (spec.initial ?? []);
      const withColumns = [
        ...initial,
        {
          name: "columns",
          source: "",
          plain: JSON.stringify(["название"]), // i18n-ignore: имя столбца ядра
          expression: false,
          from: 0,
          to: 0,
        },
      ];
      setParamForm({
        spec: { ...spec, initial: withColumns },
        title: elementById(t, "select")?.name ?? spec.typst,
        mode: { kind: "insert" },
        block: true,
      });
    },
    [client, t],
  );
  /**
   * Вставка элемента из меню или с панели. Элемент, у которого ядро велит
   * открыть форму (contracts § «Когда форма»), идёт через неё; остальные —
   * сниппетом, как раньше (FR-003).
   */
  const insertElement = useCallback(
    async (request: InsertRequest) => {
      if (request.id === "select") {
        void openSelectForm();
        return;
      }
      setInsertMenu(null);
      // Какой это элемент и какие у него параметры, ядро узнаёт по самому
      // шаблону — разбором Typst, а не по списку в ядре.
      const spec = request.id
        ? await client.elementSchemaFor(parseSnippet(request.template).text)
        : undefined;
      if (spec?.form_on_insert && request.id) {
        setParamForm({
          spec,
          title: elementById(t, request.id)?.name ?? spec.typst,
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
    [applyTemplate, client, t, openSelectForm],
  );

  /** Текст заметки сейчас — форме, чтобы ядро сверяло элемент и метки. */
  const noteText = useCallback(
    () => editorRef.current?.view()?.state.doc.toString() ?? document.text(),
    [document],
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
      const text = view?.state.doc.toString() ?? document.text();
      const at = offset ?? view?.state.selection.main.head ?? caretRef.current.offset;
      const found = await client.elementAt(text, at);
      if (found.kind === "None") {
        contextMenu.notify(t("editor.params.nothingToEdit"));
        return;
      }
      if (found.kind === "Unavailable") {
        contextMenu.notify(describeUnavailable(t, found.reason));
        return;
      }
      const spec = await client.elementSchema(found.element.id);
      if (!spec) return;
      setInsertMenu(null);
      setParamForm({
        spec,
        // Имя функции Typst: правится любой вызов, а не только записи каталога.
        title: spec.typst,
        mode: { kind: "edit", parsed: found.element },
        block: false,
      });
    },
    [client, document, contextMenu, t],
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



  const lastSelectRequest = useRef(selectRequest?.id ?? 0);
  useEffect(() => {
    if (selectRequest === null || selectRequest.id === lastSelectRequest.current) return;
    lastSelectRequest.current = selectRequest.id;
    if (active) void openSelectForm(selectRequest.conditions);
  }, [selectRequest, openSelectForm, active]);

  /**
   * Вставка блока выборки: библиотека `select.typ` кладётся в vault, текст
   * блока вставляется там же, где курсор, и одной транзакцией добавляется
   * строка `#import` (research.md §10). Отказ записи библиотеки не вставляет
   * блок, который не соберётся.
   */
  const insertSelectBlock = useCallback(
    async (text: string) => {
      const view = editorRef.current?.view();
      if (!view) return;
      try {
        onLibraryStatus?.(await client.properties.ensureSelectLibrary());
      } catch {
        setParamForm(null);
        contextMenu.notify(t("properties.libraryFailed"));
        return;
      }
      const doc = view.state.doc.toString();
      const imports = await client.properties.selectImportEdits(doc);
      const range = view.state.selection.main;
      let from = slashAt.current ?? range.from;
      let to = slashAt.current === null ? range.to : Math.max(range.to, from + 1);
      slashAt.current = null;
      // Запись свойств остаётся первым элементом заметки, а `#import` идёт сразу за
      // ней: блок встаёт не выше этой шапки, иначе `select` был бы вызван до импорта.
      const props = await client.properties.read(doc);
      let floor = imports.reduce((end, edit) => Math.max(end, edit.to), 0);
      if (props.record !== undefined) {
        const recordLine = view.state.doc.lineAt(Math.min(props.record.to, doc.length));
        floor = Math.max(floor, Math.min(recordLine.to + 1, doc.length));
      }
      const clamped = from < floor;
      if (clamped) {
        from = floor;
        to = floor;
      }
      const lineStart = view.state.doc.lineAt(Math.min(from, doc.length)).from;
      const needsBreak = !clamped && view.state.sliceDoc(lineStart, from).trim() !== "";
      const insert = needsBreak ? `\n${text}\n` : `${text}\n`;
      view.dispatch({
        changes: [
          ...imports.map((edit) => ({ from: edit.from, to: edit.to, insert: edit.insert })),
          { from, to, insert },
        ],
        userEvent: "input",
        scrollIntoView: true,
      });
      setParamForm(null);
      view.focus();
    },
    [client, contextMenu, t, onLibraryStatus],
  );

  // Запрос из палитры команд (FR-006): счётчик сменился — открыть «Параметры».
  // Реагирует только активная вкладка: при двух группах форма не откроется дважды.
  // Счётчик запоминается и неактивной, чтобы старый запрос не сработал, когда
  // вкладка станет активной.
  const lastParamsRequest = useRef(paramsRequest);
  useEffect(() => {
    if (paramsRequest === lastParamsRequest.current) return;
    lastParamsRequest.current = paramsRequest;
    if (active) void openParams();
  }, [paramsRequest, openParams, active]);

  // Панель вставки (макет § Панель вставки). Показана по умолчанию и убирается
  // кнопкой в тулбаре заметки: она для мыши и меню не заменяет.
  const [panelVisible, togglePanel] = useInsertPanelVisible();

  /**
   * Вставка с панели идёт тем же путём, что из меню: элемент с параметрами
   * открывает диалог, остальные вставляются сниппетом. Второй дороги к тексту
   * здесь не заводится.
   */
  const insertFromPanel = useCallback(
    (element: LocalizedElement) => {
      void insertElement({
        id: element.id,
        template: element.template,
        block: element.block ?? false,
      });
    },
    [insertElement],
  );

  const notePath = document.path;
  const embeds = useNoteEmbeds(client, notePath, content);

  // Доступ к редактору объявляется наверх, пока заметка открыта. Функция, а не
  // значение: она спрашивает `editorRef` в момент вставки и потому не может
  // указывать на редактор предыдущей заметки.
  const getView = useCallback(() => editorRef.current?.view() ?? null, []);
  useEffect(() => {
    if (!active) return undefined;
    onEditorReady?.(getView);
    return () => onEditorReady?.(null);
  }, [active, getView, onEditorReady]);

  // Место, куда надо встать после перехода («Назад», ссылка из колонки,
  // замечание сборки): каретка в строку и колонку, прокрутка, фокус. Документ
  // уже прочитан — иначе редактора ещё нет и эффект дождётся следующего кадра.
  useEffect(() => {
    if (!active || place === null || snapshot.read !== "ready") return;
    const view = editorRef.current?.view() ?? null;
    if (view === null) return;
    const doc = view.state.doc;
    const line = doc.line(Math.max(1, Math.min(place.line, doc.lines)));
    const position = Math.min(line.to, line.from + Math.max(0, place.column - 1));
    view.dispatch({ selection: { anchor: position }, scrollIntoView: true });
    view.focus();
    onPlaceShown?.();
  }, [active, place, snapshot.read, onPlaceShown]);

  // Фокус отдаётся редактору по запросу — щелчок по вкладке (SC-001).
  const lastFocusRequest = useRef(focusRequest);
  useEffect(() => {
    if (focusRequest === lastFocusRequest.current) return;
    lastFocusRequest.current = focusRequest;
    if (active && snapshot.read === "ready") editorRef.current?.focus();
  }, [focusRequest, active, snapshot.read]);

  // ⌘E открывает и закрывает меню вставки, Escape — только закрывает. Так же,
  // как ⌘K у палитры команд: слушатель на окне, потому что открыть меню нужно
  // и тогда, когда фокус не в тексте, а самого меню в дереве ещё нет.
  //
  // Сверка по `code` и `key` вместе никуда не делась — она переехала в
  // `useShortcuts`, а имя физической клавиши считает ядро: интерфейс русский, и
  // при кириллической раскладке `key` для этой клавиши — «у», а не «e».
  // Без открытой заметки вставлять некуда, поэтому и слушателя нет.
  useEffect(() => {
    if (!active) return undefined;
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
  }, [active, isInsertMenuOpen, openInsertMenu, closeInsertMenu, shortcuts, paramForm, openParams]);

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
      setIsInserting(true);
      setInsertError(null);
      try {
        // Настройки читаются прямо здесь, а не держатся в состоянии
        // редактора: вставка — редкая операция, а политика могла смениться в
        // панели секунду назад. Одно чтение маленького файла дешевле, чем
        // второе место, знающее текущую политику (фича 006, FR-002).
        const overrides = await client.vaultSettings();
        const content = document.text();
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
          const reason = written.reason;
          setInsertError(() => (tr: Translate) =>
            tr("editor.insertBoard.failedWith", { reason: reason(tr) }),
          );
          return;
        }

        const markup = await client.embedMarkup(plan);
        // Текст мог измениться, пока писался снимок: позиция считается по свежему.
        const fresh = document.text();
        const at = Math.min(caretRef.current.offset, fresh.length);
        const next = `${fresh.slice(0, at)}${markup}${fresh.slice(at)}`;
        // Вставка — такая же правка буфера, как набор текста, и уходит в файл
        // тем же путём (автосохранением документа): иначе разметка вставки жила
        // бы только в окне. Виды получают её сразу.
        document.replace(next, false);
        setIsChoosingBoard(false);
      } catch (cause) {
        setInsertError(() => describeInsertFailure(cause));
      } finally {
        setIsInserting(false);
      }
    },
    [client, notePath, document],
  );


  if (snapshot.read === "loading") {
    return (
      <section className="ts-pane ts-pane--editor" aria-label={t("editor.label")}>
        <div className="ts-placeholder">
          <p role="status">{t("common.loading")}</p>
        </div>
      </section>
    );
  }

  if (snapshot.read !== "ready") {
    return (
      <section className="ts-pane ts-pane--editor" aria-label={t("editor.label")}>
        <div className="ts-placeholder">
          <p role="alert">{describeReadError(t, snapshot.read)}</p>
        </div>
      </section>
    );
  }

  return (
    <section className="ts-pane ts-pane--editor" aria-label={t("editor.label")}>
      {/* Заголовка и «Закрыть» здесь нет: их роль у вкладки (фича 014). */}
      <div className="ts-pane__toolbar">
        <span className="ts-spacer" />
        <div className="ts-group" role="group" aria-label={t("editor.actions")}>
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
              className="ts-btn ts-btn--shrink"
              title={hintWith(
                t("editor.insertElementHint"),
                shortcuts.displayOf("insert-menu"),
              )}
              aria-haspopup="dialog"
              aria-expanded={isInsertMenuOpen}
              onClick={openInsertMenu}
            >
              {t("editor.insertElement")}
            </button>
          )}
          {/* SC-007: от открытой заметки до вставленной доски — два действия,
              эта кнопка и выбор доски в списке. */}
          <button
            type="button"
            className="ts-btn ts-btn--shrink"
            // В узкой колонке подпись сжимается многоточием (010 SC-006).
            title={t("editor.insertBoard")}
            onClick={() => {
              setInsertError(null);
              setIsChoosingBoard(true);
            }}
          >
            {t("editor.insertBoard")}
          </button>
          {/* SC-006: от открытой заметки до сохранённой заготовки — три
              действия, эта кнопка, ввод имени и подтверждение. Сохраняется
              **буфер**, а не файл с диска: заготовка фиксирует то, что
              пользователь видит перед собой (FR-027). */}
          {onSaveAsTemplate && (
            <button
              type="button"
              className="ts-btn ts-btn--shrink"
              title={t("editor.saveAsTemplate")}
              onClick={() => onSaveAsTemplate(document.text())}
            >
              {t("editor.saveAsTemplate")}
            </button>
          )}
          {/* Выключатель панели вставки. В тулбаре, а не в самой панели:
              убранную панель нечем было бы вернуть. */}
          <button
            type="button"
            className="ts-btn"
            aria-pressed={panelVisible}
            title={t("editor.panelHint")}
            onClick={togglePanel}
          >
            {t("editor.panel")}
          </button>
          {/* Кнопки «Сохранить» здесь нет намеренно: набранное уходит в файл
              само, а состояние записи видно в статус-баре. Оставить её значило
              бы завести второй путь к тому же `write` и второй ответ на вопрос
              «сохранено ли». */}
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
                ? { available: false, reason: describeUnavailable(t, caretElement.reason) }
                : null
          }
          paramsShortcut={shortcuts.displayOf("element-params")}
          onParams={() => void openParams()}
        />
      )}

      <CodeEditor
        ref={editorRef}
        key={tabId}
        document={document}
        tabId={tabId}
        ariaLabel={t("editor.contentLabel", { name })}
        {...(typst ? { highlight: typstHighlightFor(client) } : {})}
        onCaretMove={onCaretMove}
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
          onInsert={(text) =>
            paramForm.spec.id === SELECT_ELEMENT_ID
              ? void insertSelectBlock(text)
              : insertRendered(text, paramForm.block)
          }
          onApply={applyEdits}
          onCancel={closeForm}
        />
      )}

      {/* Вставки заметки и их актуальность (FR-027, FR-031, FR-032). Список, а
          не пометки внутри текста: индикатор рядом с вызовом требует редактора,
          знающего позиции в тексте, — в textarea их не разместить, а врать о
          месте хуже, чем показать список. */}
      {embeds.embeds.length > 0 && (
        <ul className="ts-embeds" aria-label={t("editor.embedsLabel")}>
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
        error={insertError === null ? null : insertError(t)}
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
        <span className="ts-vim" title={t("editor.vimHint")}>
          NORMAL
        </span>
        <span>{t("editor.caret", { line: caret.line, column: caret.column })}</span>
        <span className="ts-statusbar__minor">UTF-8</span>{/* i18n-ignore: имя кодировки */}
        {/* Состояние полей вставленного сниппета (макет «Вставка» § состояние
            полей): без счётчика подсветка говорит только «что-то выделено». */}
        {fieldState && (
          <span className="ts-fieldstate">
            {t("editor.fieldState", { index: fieldState.index, count: fieldState.count })}
          </span>
        )}
        <SaveIndicator state={snapshot.save} />
        <span className="ts-spacer" />

        {linksSummary !== undefined && <span>{linksSummary}</span>}
        {tabsSummary !== undefined && <span>{tabsSummary}</span>}

        {/* Кегль редактора — те же ступени, что у масштаба предпросмотра, и
            тот же жест Ctrl/⌘ + колесо поверх текста. Значение — сама кнопка:
            нажатие снимает перекрытие папки и возвращает размер устройства. */}
        {fontSize && (
          <div className="ts-zoom" role="group" aria-label={t("editor.fontSize.label")}>
            <button
              type="button"
              className="ts-btn ts-zoom__step"
              onClick={() => fontSize.step(-1)}
              aria-label={t("editor.fontSize.decrease")}
              title={t("editor.fontSize.decreaseHint")}
            >
              −
            </button>
            <button
              type="button"
              className="ts-btn ts-zoom__value"
              onClick={fontSize.reset}
              disabled={!fontSize.isOverridden}
              aria-label={t("editor.fontSize.value", { size: fontSize.size })}
              title={t("editor.fontSize.reset")}
            >
              {fontSize.size}&nbsp;px
            </button>
            <button
              type="button"
              className="ts-btn ts-zoom__step"
              onClick={() => fontSize.step(1)}
              aria-label={t("editor.fontSize.increase")}
              title={t("editor.fontSize.increaseHint")}
            >
              +
            </button>
          </div>
        )}

        <span>{countWords(t, content)}</span>
        <span className="ts-tag ts-statusbar__minor">typst</span>{/* i18n-ignore: имя языка разметки */}
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
  const { t } = useI18n();
  if (state.state === "Failed") {
    return (
      <p className="ts-save ts-save--failed" role="alert">
        {state.reason(t)}
      </p>
    );
  }
  const label =
    state.state === "Saving"
      ? t("editor.save.saving")
      : state.state === "Dirty"
        ? t("editor.save.dirty")
        : state.state === "Held"
          ? t("editor.save.held")
          : t("editor.save.saved");
  return <p className="ts-save">{label}</p>;
}

/**
 * Счётчик слов для статус-бара («142 слова» в макете).
 *
 * Считает по пробелам, вместе с разметкой Typst: отделять команды от текста
 * значило бы разбирать язык, а разбор живёт в компиляторе, не здесь.
 */
function countWords(t: Translate, text: string): string {
  const count = text.split(/\s+/).filter((word) => word.length > 0).length;
  return t("editor.words", { count });
}

/** Почему вставка не состоялась (FR-018, FR-019). */
function describeInsertFailure(cause: unknown): Phrase {
  const kind =
    typeof cause === "object" && cause !== null && "kind" in cause
      ? (cause as { kind: unknown }).kind
      : null;
  switch (kind) {
    case "NotANote":
      return (t) => t("editor.insertBoard.notANote");
    case "NotABoard":
      return (t) => t("editor.insertBoard.notABoard");
    case "NotFound":
      return (t) => t("editor.insertBoard.notFound");
    case "Unreadable":
      return (t) => t("editor.insertBoard.unreadable");
    default:
      return (t) => t("editor.insertBoard.failed");
  }
}

/**
 * Причина отказа чтения называется словами (FR-008, FR-014). `TooLarge` —
 * отдельный случай: заметка существует и видна в дереве, отказано именно в
 * открытии, и пользователю важно понимать разницу.
 */
function describeReadError(t: Translate, error: ReadError): string {
  switch (error.kind) {
    case "NotFound":
      return t("editor.read.notFound");
    case "TooLarge":
      return t("editor.read.tooLarge");
    case "Io":
      return t("editor.read.io");
    default: {
      const exhaustive: never = error;
      return exhaustive;
    }
  }
}
