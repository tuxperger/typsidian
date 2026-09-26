"use client";

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type Dispatch,
  type SetStateAction,
} from "react";
import type {
  CompiledDocument,
  Diagnostic,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";
import type { PreviewClient } from "../preview-client.js";
import { PreviewPage } from "./PreviewPage.js";
import { describeInstallError } from "./describeInstallError.js";
import { copyText, useContextMenu } from "../menu/ContextMenu.js";

// Принцип I: панель ничего не решает про документ. «Собрался или нет»,
// «где ошибка», «какие страницы» — всё приходит из ядра готовым. Здесь только
// показ и ритм запросов.

/**
 * Пауза после последнего нажатия, по истечении которой запрашивается сборка.
 *
 * Дебаунс живёт в UI, потому что он про ритм ввода, а не про правила сборки
 * (contracts/frontend-preview.md). Значение выбрано под SC-002 «обновление ≤1 с
 * после остановки»: пауза плюс сама сборка обязаны уложиться в бюджет.
 */
export const PREVIEW_DEBOUNCE_MS = 250;

/**
 * Ступени масштаба листа.
 *
 * Список, а не свободный множитель: масштаб меняют и кнопки, и колесо, и обе
 * дороги обязаны попадать в одни и те же значения — иначе «100 %» перестаёт
 * быть достижимой точкой возврата.
 *
 * `1` — базовая раскладка: лист занимает **всю** ширину колонки. Раньше он был
 * ограничен 520px из макета, и расширение колонки уходило в пустое поле справа
 * — при 1:1 в широком окне лист занимал меньше половины отведённого ему места.
 * Проценты отсчитываются от этой раскладки, а не от физического размера
 * страницы: настоящий сантиметр на экране требовал бы плотности точек монитора,
 * которой браузер не сообщает.
 */
const ZOOM_STEPS = [0.5, 0.67, 0.8, 0.9, 1, 1.15, 1.3, 1.5, 1.75, 2, 2.5, 3];
const ZOOM_DEFAULT = ZOOM_STEPS.indexOf(1);

/** Что показывает индикатор сборки в шапке (handoff § Header bar). */
export interface CompileStatus {
  isBuilding: boolean;
  /** Сколько заняла последняя сборка; `null` — её ещё не было. */
  lastMs: number | null;
}

export interface PreviewPaneProps {
  client: PreviewClient | null;
  notePath: string | null;
  /** Текст из буфера редактора, включая несохранённые правки (FR-002). */
  text: string;
  /**
   * Состояние сборки — наверх, в шапку. Панель остаётся единственным местом,
   * которое сборку запускает: два независимых цикла компиляции на один
   * документ означали бы две очереди к ядру.
   */
  onStatus?: (status: CompileStatus) => void;
  /**
   * Счётчик ручных пересборок. Меняется — панель собирает заново, не дожидаясь
   * правки текста: кнопка в шапке иначе ничего бы не делала при неизменном
   * документе.
   */
  rebuildToken?: number;
  /**
   * Настройки уровня vault (фича 007). Если переданы и клиент умеет
   * доустанавливать шаблоны, сборка идёт путём, который может получить
   * недостающий шаблон из сети.
   *
   * `null` — vault не открыт либо настройки ещё не прочитаны: тогда обычная
   * сборка, без обращений в сеть. Решение «идти ли в сеть» принимает ядро по
   * этим значениям, панель их только передаёт (принцип I).
   */
  vaultOverrides?: VaultOverrides | null;
}

/** Что показывает панель прямо сейчас (data-model.md «PreviewState»). */
interface PreviewState {
  /** Последний **успешно** собранный документ. */
  shown: CompiledDocument | null;
  /** `shown` не соответствует текущему тексту (FR-014). */
  isStale: boolean;
  isBuilding: boolean;
  diagnostics: Diagnostic[];
}

const INITIAL: PreviewState = {
  shown: null,
  isStale: false,
  isBuilding: false,
  diagnostics: [],
};

export function PreviewPane({
  client,
  notePath,
  text,
  onStatus,
  rebuildToken = 0,
  vaultOverrides = null,
}: PreviewPaneProps): React.JSX.Element {
  const [state, setState] = useState<PreviewState>(INITIAL);
  const [lastMs, setLastMs] = useState<number | null>(null);
  // Масштаб — дело показа, а не документа: сборка от него не зависит, поэтому
  // он живёт здесь и переживает переключение заметок, как и в просмотрщиках.
  const [zoomIndex, setZoomIndex] = useState(ZOOM_DEFAULT);
  const previewRef = useRef<HTMLDivElement | null>(null);
  // Ширина, отведённая листу. Меряется, а не считается в CSS: её знает только
  // раскладка — колонка минус линейка, минус зазор, минус полоса прокрутки, — а
  // вертикальной линейке та же величина нужна в пикселях, потому что её деления
  // абсолютные. Одно измерение на обоих и есть то, что держит их вместе.
  const stackRef = useRef<HTMLDivElement | null>(null);
  const [pageWidth, setPageWidth] = useState<number | null>(null);
  // Колбэк в ref: новая ссылка на него не должна перезапускать сборку.
  const onStatusRef = useRef(onStatus);
  onStatusRef.current = onStatus;
  // Монотонная версия текста: результат более старой сборки не должен вытеснять
  // более новый (FR-009). Ядро помечает результат этой же версией.
  const revisionRef = useRef(0);
  const shownRevisionRef = useRef(-1);
  // Явная загрузка шаблона (007 FR-010). Счётчик — чтобы после установки
  // пересобрать заметку сразу, не дожидаясь правки текста.
  const [installRound, setInstallRound] = useState(0);
  // Пересборка из контекстного меню — тот же приём, что после установки шаблона.
  const [menuRound, setMenuRound] = useState(0);
  const contextMenu = useContextMenu();
  const [installing, setInstalling] = useState<string | null>(null);
  const [installNote, setInstallNote] = useState<{
    ok: boolean;
    text: string;
  } | null>(null);

  // Сообщение об установке относится к заметке, из которой её попросили.
  useEffect(() => {
    setInstallNote(null);
  }, [notePath]);

  const install = useCallback(
    async (spec: string) => {
      if (!client?.installTemplate || !vaultOverrides) return;
      setInstalling(spec);
      try {
        const report = await client.installTemplate(spec, vaultOverrides);
        // FR-006: что и откуда — и при явной загрузке тоже.
        setInstallNote({
          ok: true,
          text: `Шаблон ${report.spec} загружен с ${report.url} (${report.file_count} файлов, отпечаток ${report.fingerprint.slice(0, 12)}).`,
        });
        setInstallRound((round) => round + 1);
      } catch (cause) {
        setInstallNote({ ok: false, text: describeInstallError(cause) });
      } finally {
        setInstalling(null);
      }
    },
    [client, vaultOverrides],
  );
  const canInstall = Boolean(client?.installTemplate && vaultOverrides);

  useEffect(() => {
    if (!client || notePath === null) {
      setState(INITIAL);
      shownRevisionRef.current = -1;
      return;
    }

    const revision = ++revisionRef.current;
    setState((prev) => ({ ...prev, isBuilding: true }));

    const timer = window.setTimeout(() => {
      void (async () => {
        // Время меряется вокруг вызова ядра: это ровно то, сколько пользователь
        // ждал результат. Своего поля с длительностью `CompiledDocument` не
        // несёт, а выдумывать его в ядре ради надписи незачем.
        const startedAt = performance.now();
        try {
          // Путь с доустановкой шаблонов — только когда клиент его умеет и
          // настройки vault прочитаны. Иначе обычная сборка: заметка без
          // подключений в обоих случаях в сеть не заходит (007 FR-008).
          const document =
            client.compileWithTemplates && vaultOverrides
              ? await client.compileWithTemplates(
                  notePath,
                  text,
                  revision,
                  vaultOverrides,
                )
              : await client.compile(notePath, text, revision);
          setLastMs(Math.round(performance.now() - startedAt));
          // Пришёл результат для текста, который уже устарел, — выбрасываем.
          if (document.revision < shownRevisionRef.current) return;
          shownRevisionRef.current = document.revision;

          setState((prev) => ({
            // Успешная сборка заменяет вид; неуспешная оставляет прежний и
            // помечает его несоответствующим тексту (FR-014).
            shown: document.outcome === "Succeeded" ? document : prev.shown,
            isStale: document.outcome !== "Succeeded",
            isBuilding: false,
            diagnostics: document.diagnostics,
          }));
        } catch (cause) {
          setState((prev) => ({
            ...prev,
            isBuilding: false,
            isStale: true,
            diagnostics: [
              {
                severity: "Error",
                message: `Сборка не состоялась: ${String(cause)}`,
                location: undefined,
              },
            ],
          }));
        }
      })();
    }, PREVIEW_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [client, notePath, text, rebuildToken, vaultOverrides, installRound, menuRound]);

  // Шапка узнаёт о сборке отсюда, а не считает её сама.
  useEffect(() => {
    onStatusRef.current?.({ isBuilding: state.isBuilding, lastMs });
  }, [state.isBuilding, lastMs]);

  // Ctrl/⌘ + колесо — тот же жест, что в любом просмотрщике документов.
  // Слушатель вешается вручную: `onWheel` React ставит пассивным, а пассивный
  // обработчик не может отменить масштабирование всей страницы браузером.
  useEffect(() => {
    const node = previewRef.current;
    if (!node) return undefined;

    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      shiftZoom(setZoomIndex, event.deltaY < 0 ? 1 : -1);
    };

    node.addEventListener("wheel", onWheel, { passive: false });
    return () => node.removeEventListener("wheel", onWheel);
  }, []);

  // Дробная часть отбрасывается: измерения приходят субпиксельными, и без
  // округления состояние менялось бы на сотых долях пикселя. Ширина листа на
  // раскладку не влияет (лист шире колонки просто выезжает за неё, а полоса
  // прокрутки зарезервирована постоянно), поэтому обратной связи «шире лист →
  // уже колонка → уже лист» здесь нет, а при неизменной ширине состояние не
  // двигается вовсе — и цикла отрисовок отсюда не берётся.
  const measurePage = useCallback(() => {
    const node = stackRef.current;
    if (node === null) return;
    setPageWidth((previous) => {
      const next = Math.floor(node.clientWidth);
      return next > 0 && next !== previous ? next : previous;
    });
  }, []);

  // Слежение за шириной колонки. Один `ResizeObserver` на всё: он сообщает о
  // любом изменении размера — и о том, что сделал React (границу колонок
  // тянут, дерево файлов прячут), и о том, что сделал браузер (размер окна,
  // масштаб страницы), — и первый вызов приходит сразу при `observe`.
  //
  // Замера после каждой отрисовки здесь быть не должно, хотя соблазн есть:
  // эффект без списка зависимостей, зовущий `setState`, React считает
  // бесконечным обновлением и рвёт отрисовку с «Maximum update depth exceeded»
  // — бездействие при неизменной ширине его не спасает.
  useEffect(() => {
    const node = stackRef.current;
    if (node === null || typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(measurePage);
    observer.observe(node);
    return () => observer.disconnect();
  }, [measurePage]);

  const errors = useMemo(
    () => state.diagnostics.filter((d) => d.severity === "Error"),
    [state.diagnostics],
  );
  const warnings = useMemo(
    () => state.diagnostics.filter((d) => d.severity === "Warning"),
    [state.diagnostics],
  );

  const zoom = ZOOM_STEPS[zoomIndex] ?? 1;
  const percent = Math.round(zoom * 100);

  return (
    <section className="ts-pane ts-pane--preview" aria-label="Предпросмотр">
      <div className="ts-pane__toolbar">
        <span className="ts-eyebrow">Предпросмотр</span>
        <span className="ts-pane__meta ts-pane__meta--sep">·</span>
        <span className="ts-pane__meta">A4 · 21×29.7 см</span>
        <span className="ts-spacer" />
        {state.isBuilding && <span className="ts-tag">собирается…</span>}
        {state.isStale && !state.isBuilding && (
          <span className="ts-tag">не соответствует тексту</span>
        )}

        <div className="ts-zoom" role="group" aria-label="Масштаб предпросмотра">
          <button
            type="button"
            className="ts-btn ts-zoom__step"
            onClick={() => shiftZoom(setZoomIndex, -1)}
            disabled={zoomIndex === 0}
            aria-label="Уменьшить масштаб"
            title="Уменьшить масштаб (Ctrl + колесо)"
          >
            −
          </button>
          {/* Величина — сама кнопка: возврат к 100 % нужен чаще всего, а
              отдельная третья кнопка ради него распирала бы строку 26px. */}
          <button
            type="button"
            className="ts-btn ts-zoom__value"
            onClick={() => setZoomIndex(ZOOM_DEFAULT)}
            aria-label={`Масштаб ${percent}%, вернуть 100%`}
            title="Вернуть 100 %"
          >
            {percent}&nbsp;%
          </button>
          <button
            type="button"
            className="ts-btn ts-zoom__step"
            onClick={() => shiftZoom(setZoomIndex, 1)}
            disabled={zoomIndex === ZOOM_STEPS.length - 1}
            aria-label="Увеличить масштаб"
            title="Увеличить масштаб (Ctrl + колесо)"
          >
            +
          </button>
        </div>
      </div>

      {/* Масштаб уходит в CSS переменной: ширину листа и деления линеек считает
          раскладка, а не JS — пересчитывать их в React значило бы дублировать
          то, что браузер уже умеет через `calc`. */}
      <div
        className="ts-preview"
        ref={previewRef}
        onContextMenu={(event) => {
          // Выделенный текст берётся из текстового слоя страницы (FR-031): он
          // настоящий, поэтому копируется то же, что видно выделенным.
          const selection = window.getSelection();
          const node = previewRef.current;
          const selected =
            selection !== null &&
            node !== null &&
            !selection.isCollapsed &&
            node.contains(selection.anchorNode)
              ? selection.toString()
              : "";
          contextMenu.open(event, [
            {
              label: "Копировать",
              disabled: selected === "",
              run: async () => {
                const failure = await copyText(selected);
                if (failure) contextMenu.notify(failure);
              },
            },
            { kind: "separator" },
            {
              label: "Пересобрать",
              disabled: !client || notePath === null,
              run: () => setMenuRound((round) => round + 1),
            },
            { kind: "separator" },
            {
              label: "Увеличить",
              disabled: zoomIndex === ZOOM_STEPS.length - 1,
              run: () => shiftZoom(setZoomIndex, 1),
            },
            {
              label: "Уменьшить",
              disabled: zoomIndex === 0,
              run: () => shiftZoom(setZoomIndex, -1),
            },
            {
              label: "Масштаб 100 %",
              disabled: zoomIndex === ZOOM_DEFAULT,
              run: () => setZoomIndex(ZOOM_DEFAULT),
            },
          ]);
        }}
        style={
          {
            "--ts-preview-zoom": zoom,
            // До первого измерения (и на сервере, где мерить нечего) остаётся
            // запасная ширина из стилей: лист обязан быть осмысленным уже на
            // первом кадре.
            ...(pageWidth === null ? {} : { "--ts-page-w": `${pageWidth}px` }),
          } as CSSProperties
        }
      >
        {/* Линейки в сантиметрах — мотив макета (handoff § Ruler motif):
            признак того, что перед пользователем постраничный документ, а не
            бесконечная лента.

            Горизонтальная повторяет ширину листа (CSS `.ts-ruler-h`), поэтому
            деления стоят в долях от 21 см A4: 5 см — это 23.8 % ширины, а не
            «на глаз». Вертикальная своей высоты не имеет, её деления считаются
            от той же ширины листа тем же множителем масштаба. */}
        <div className="ts-ruler-h" aria-hidden="true">
          <span style={{ left: "0" }}>0</span>
          <span style={{ left: "23.8%" }}>5</span>
          <span style={{ left: "47.6%" }}>10</span>
          <span style={{ left: "71.4%" }}>15</span>
          <span style={{ left: "95.2%" }}>20</span>
        </div>

        <div className="ts-preview__rows">
          <div className="ts-ruler-v" aria-hidden="true">
            <span style={{ top: "0" }}>0</span>
            <span style={{ top: "var(--ts-ruler-10cm)" }}>10</span>
            <span style={{ top: "calc(var(--ts-ruler-10cm) * 2)" }}>20</span>
          </div>

          <div className="ts-preview__stack" ref={stackRef}>
            {/* Ошибки — на месте предпросмотра, а не строчкой внизу: собранного
                вида всё равно нет или он устарел, и главное на панели сейчас
                они. Список остаётся полным и в порядке появления (FR-016). */}
            {errors.length > 0 && (
              <div className="ts-failure" role="alert">
                <p className="ts-failure__head">
                  {state.shown
                    ? "Заметка не собралась — показан прошлый вид"
                    : "Показывать нечего: заметка ещё ни разу не собралась"}
                </p>
                <ul className="ts-failure__list">
                  {errors.map((d, i) => (
                    <DiagnosticItem
                      key={`e${i}`}
                      diagnostic={d}
                      onInstall={canInstall ? install : undefined}
                      installing={installing}
                    />
                  ))}
                </ul>
              </div>
            )}

            {state.shown ? (
              state.shown.pages.map((page) => <PreviewPage key={page.index} page={page} />)
            ) : errors.length === 0 ? (
              // Пустая панель без объяснения запрещена (FR-014, SC-004).
              <div className="ts-placeholder">
                {notePath === null ? (
                  <p>Откройте заметку, чтобы увидеть её свёрстанный вид.</p>
                ) : (
                  <p>Собирается…</p>
                )}
              </div>
            ) : null}
          </div>
        </div>
      </div>

      {installNote && (
        <p
          className="ts-diagnostics ts-diagnostics__install"
          role={installNote.ok ? "status" : "alert"}
        >
          {installNote.text}
        </p>
      )}

      {/* Предупреждения сборку не ломают (FR-018) и остаются внизу: поднимать
          их на место документа значило бы приравнять к ошибкам. */}
      {warnings.length > 0 && (
        <ul className="ts-diagnostics" aria-label="Предупреждения">
          {warnings.map((d, i) => (
            <DiagnosticItem key={`w${i}`} diagnostic={d} />
          ))}
        </ul>
      )}
    </section>
  );
}

/**
 * Сдвиг масштаба на одну ступень с упором в края списка.
 *
 * Общая для кнопок и колеса: край обязан вести себя одинаково, а зацикливание
 * с 300 % на 50 % в просмотрщике документов означало бы потерю места в тексте.
 */
function shiftZoom(
  setZoomIndex: Dispatch<SetStateAction<number>>,
  direction: 1 | -1,
): void {
  setZoomIndex((index) =>
    Math.min(ZOOM_STEPS.length - 1, Math.max(0, index + direction)),
  );
}

function DiagnosticItem({
  diagnostic,
  onInstall,
  installing = null,
}: {
  diagnostic: Diagnostic;
  /**
   * Явная загрузка шаблона, названного замечанием (007 FR-010). Кнопка есть
   * только у замечаний, к которым ядро приложило спецификацию шаблона, — какой
   * шаблон отсутствует, решает ядро, а не разбор текста здесь.
   */
  onInstall?: ((spec: string) => void | Promise<void>) | undefined;
  /** Какой шаблон ставится прямо сейчас. */
  installing?: string | null;
}): React.JSX.Element {
  const template = diagnostic.template;
  const place = diagnostic.location;
  return (
    <li
      className={
        diagnostic.severity === "Error"
          ? "ts-diagnostics__error"
          : "ts-diagnostics__warning"
      }
    >
      <span>{diagnostic.message}</span>
      {place && (
        <span className="ts-diagnostics__place">
          {" "}
          — {place.path}:{place.line}:{place.column}
        </span>
      )}
      {template && onInstall && (
        <>
          {" "}
          <button
            type="button"
            className="ts-btn"
            disabled={installing !== null}
            onClick={() => void onInstall(template)}
          >
            {installing === template ? "Загружается…" : "Загрузить шаблон"}
          </button>
        </>
      )}
    </li>
  );
}
