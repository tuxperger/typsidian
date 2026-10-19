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
  Outline,
  Diagnostic,
  DiagnosticLocation,
  VaultOverrides,
} from "../wasm-core/typsidian_core.js";
import type { PreviewClient } from "../preview-client.js";
import { Icon } from "../icons/Icons.js";
import { PreviewPage } from "./PreviewPage.js";
import { useCompiledDocument, type PreviewState } from "./useCompiledDocument.js";
import { useScrollAnchor } from "./useScrollAnchor.js";
import { describeInstallError } from "./describeInstallError.js";
import { diagnosticText } from "../i18n/messages.js";
import { useI18n, type Phrase } from "../i18n/index.js";
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
export { PREVIEW_DEBOUNCE_MS } from "./useCompiledDocument.js";

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
  /**
   * Экспорт открытой заметки в PDF (фича 009, FR-005). `undefined` — кнопки
   * нет: экспорт недоступен этому клиенту или открыта не заметка.
   */
  onExport?: (() => void) | undefined;
  /** Экспорт идёт — кнопка недоступна, пока не пришёл итог. */
  exporting?: boolean;
  /**
   * Переход к месту замечания в редакторе (FR-015). Открыть связанный файл и
   * поставить курсор умеет рабочая область — панель только просит.
   */
  onReveal?: ((place: DiagnosticLocation) => void) | undefined;
  /**
   * Можно ли открыть файл в редакторе. Место в файле, который редактором не
   * открывается (шрифт, изображение, файл шаблона), остаётся строкой без
   * действия — кнопка, которая ничего не делает, хуже её отсутствия.
   */
  canReveal?: ((path: string) => boolean) | undefined;
  /**
   * Файлы vault, от которых зависит последняя сборка, — рабочая область
   * пересобирает заметку, когда один из них меняется (FR-020).
   */
  onDependencies?: ((paths: string[]) => void) | undefined;
  /**
   * Состояние сборки наверх — для показа слайдов (фича 019): ему нужны последний
   * удачный документ, пометка «устарел» и замечания. Сборку по-прежнему ведёт
   * только эта панель.
   */
  onBuild?: ((notePath: string, state: PreviewState) => void) | undefined;
  /**
   * Показать заголовок пункта оглавления (фича 015, FR-004). Новый `seq` —
   * новый запрос; где заголовок в документе, ищет ядро по текстовому слою.
   * Не нашлось или документа нет — предпросмотр не двигается.
   */
  headingRequest?: HeadingRequest | null;
}

/** Запрос «показать заголовок» от оглавления. */
export interface HeadingRequest {
  notePath: string;
  outline: Outline;
  key: string;
  seq: number;
}


export function PreviewPane({
  client,
  notePath,
  text,
  onStatus,
  rebuildToken = 0,
  vaultOverrides = null,
  onExport,
  exporting = false,
  onReveal,
  canReveal,
  onDependencies,
  onBuild,
  headingRequest = null,
}: PreviewPaneProps): React.JSX.Element {
  const { t } = useI18n();
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
  // Сборка, её состояние и всё, что вокруг неё, — в хуке; панель показывает.
  const { state, lastMs, rebuild } = useCompiledDocument({
    client,
    notePath,
    text,
    rebuildToken,
    vaultOverrides,
    scroller: previewRef,
    onDependencies,
  });
  // Переход из оглавления (фича 015): тот же механизм прокрутки к прогону, что
  // удерживает место при пересборке. Документ — последний показанный: ждать
  // свежей сборки переход не должен (FR-004 «в пределах возможностей 005»).
  const headingAnchor = useScrollAnchor(previewRef);
  const shownRef = useRef(state.shown);
  shownRef.current = state.shown;
  const headingSeq = headingRequest?.seq ?? 0;
  useEffect(() => {
    const document = shownRef.current;
    if (headingRequest === null || client === null || document === null) return undefined;
    if (headingRequest.notePath !== notePath) return undefined;
    let stale = false;
    void client
      .locateHeading(
        document.pages.map((page) => page.text_runs),
        headingRequest.outline,
        headingRequest.key,
      )
      .then((resolved) => {
        if (!stale && resolved !== null) headingAnchor.restore(resolved, 0);
      })
      .catch(() => undefined);
    return () => {
      stale = true;
    };
    // Запрос — по номеру: тот же пункт, выбранный дважды, прокручивает дважды.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [headingSeq]);
  const onBuildRef = useRef(onBuild);
  onBuildRef.current = onBuild;
  useEffect(() => {
    if (notePath !== null) onBuildRef.current?.(notePath, state);
  }, [notePath, state]);
  const contextMenu = useContextMenu();
  const [installing, setInstalling] = useState<string | null>(null);
  const [installNote, setInstallNote] = useState<{
    ok: boolean;
    text: Phrase;
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
          text: (tr) =>
            tr("app.templateDownloaded", {
              spec: report.spec,
              url: report.url,
              fileCount: report.file_count,
              fingerprint: report.fingerprint,
            }),
        });
        // После установки заметка пересобирается сразу, не дожидаясь правки
        // текста (007 FR-010).
        rebuild();
      } catch (cause) {
        setInstallNote({ ok: false, text: describeInstallError(cause) });
      } finally {
        setInstalling(null);
      }
    },
    [client, vaultOverrides, rebuild],
  );
  const canInstall = Boolean(client?.installTemplate && vaultOverrides);

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
    <section className="ts-pane ts-pane--preview" aria-label={t("preview.label")}>
      <div className="ts-pane__toolbar">
        <span className="ts-eyebrow">{t("preview.label")}</span>
        <span className="ts-pane__meta ts-pane__meta--sep">·</span>
        <span className="ts-pane__meta" title={t("preview.pageSize")}>{t("preview.pageSize")}</span>
        <span className="ts-spacer" />
        {/* В узкой колонке метка сжимается многоточием — полный текст в подсказке. */}
        {state.isBuilding && <span className="ts-tag" title={t("preview.building")}>{t("preview.building")}</span>}
        {state.isStale && !state.isBuilding && (
          <span className="ts-tag" title={t("preview.stale")}>{t("preview.stale")}</span>
        )}

        <div className="ts-zoom" role="group" aria-label={t("preview.zoom.label")}>
          <button
            type="button"
            className="ts-btn ts-zoom__step"
            onClick={() => shiftZoom(setZoomIndex, -1)}
            disabled={zoomIndex === 0}
            aria-label={t("preview.zoom.out")}
            title={t("preview.zoom.outHint")}
          >
            −
          </button>
          {/* Величина — сама кнопка: возврат к 100 % нужен чаще всего, а
              отдельная третья кнопка ради него распирала бы строку 26px. */}
          <button
            type="button"
            className="ts-btn ts-zoom__value"
            onClick={() => setZoomIndex(ZOOM_DEFAULT)}
            aria-label={t("preview.zoom.value", { percent })}
            title={t("preview.zoom.reset")}
          >
            {percent}&nbsp;%
          </button>
          <button
            type="button"
            className="ts-btn ts-zoom__step"
            onClick={() => shiftZoom(setZoomIndex, 1)}
            disabled={zoomIndex === ZOOM_STEPS.length - 1}
            aria-label={t("preview.zoom.in")}
            title={t("preview.zoom.inHint")}
          >
            +
          </button>
        </div>

        {onExport !== undefined && (
          <button
            type="button"
            className="ts-btn"
            onClick={onExport}
            disabled={exporting || notePath === null}
            title={t("workspace.exportPdf")}
          >
            <Icon name="download" size={11} />
            {exporting ? "PDF…" : "PDF"}
          </button>
        )}
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
              label: t("editor.menu.copy"),
              disabled: selected === "",
              run: async () => {
                const failure = await copyText(selected);
                if (failure) contextMenu.notify(failure);
              },
            },
            { kind: "separator" },
            {
              label: t("workspace.rebuild"),
              disabled: !client || notePath === null,
              run: rebuild,
            },
            { kind: "separator" },
            {
              label: t("preview.zoom.inShort"),
              disabled: zoomIndex === ZOOM_STEPS.length - 1,
              run: () => shiftZoom(setZoomIndex, 1),
            },
            {
              label: t("preview.zoom.outShort"),
              disabled: zoomIndex === 0,
              run: () => shiftZoom(setZoomIndex, -1),
            },
            {
              label: t("preview.zoom.hundred"),
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
            {(errors.length > 0 || state.failure !== null) && (
              <div className="ts-failure" role="alert">
                <p className="ts-failure__head">
                  {state.shown
                    ? t("preview.failedShowingPrevious")
                    : t("preview.failedNothingToShow")}
                </p>
                <ul className="ts-failure__list">
                  {state.failure !== null && (
                    <li className="ts-diagnostics__error">
                      {t("preview.buildFailed", { detail: state.failure })}
                    </li>
                  )}
                  {errors.map((d, i) => (
                    <DiagnosticItem
                      key={`e${i}`}
                      diagnostic={d}
                      onInstall={canInstall ? install : undefined}
                      installing={installing}
                      onReveal={onReveal}
                      canReveal={canReveal}
                    />
                  ))}
                </ul>
              </div>
            )}

            {state.shown ? (
              state.shown.pages.map((page) => <PreviewPage key={page.index} page={page} />)
            ) : errors.length === 0 && state.failure === null ? (
              // Пустая панель без объяснения запрещена (FR-014, SC-004).
              <div className="ts-placeholder">
                {notePath === null ? (
                  <p>{t("preview.openNote")}</p>
                ) : (
                  <p>{t("preview.buildingLong")}</p>
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
          {installNote.text(t)}
        </p>
      )}

      {/* Предупреждения сборку не ломают (FR-018) и остаются внизу: поднимать
          их на место документа значило бы приравнять к ошибкам. */}
      {warnings.length > 0 && (
        <ul className="ts-diagnostics" aria-label={t("preview.warnings")}>
          {warnings.map((d, i) => (
            <DiagnosticItem
              key={`w${i}`}
              diagnostic={d}
              onReveal={onReveal}
              canReveal={canReveal}
            />
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
  onReveal,
  canReveal,
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
  onReveal?: ((place: DiagnosticLocation) => void) | undefined;
  canReveal?: ((path: string) => boolean) | undefined;
}): React.JSX.Element {
  const { t } = useI18n();
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
      {/* Сообщение приложения — на языке интерфейса, Typst — как есть
          (010 FR-004); текст собирается при рендере (FR-014). */}
      <span>{diagnosticText(t, diagnostic)}</span>
      {place && (
        <>
          {" — "}
          {onReveal && canReveal?.(place.path) ? (
            <button
              type="button"
              className="ts-diagnostics__place"
              title={t("preview.reveal", { place: placeText(place) })}
              onClick={() => onReveal(place)}
            >
              {placeText(place)}
            </button>
          ) : (
            <span className="ts-diagnostics__place">{placeText(place)}</span>
          )}
        </>
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
            {installing === template ? t("preview.installing") : t("preview.install")}
          </button>
        </>
      )}
    </li>
  );
}

/** Место замечания строкой: `файл:строка:колонка`. */
function placeText(place: DiagnosticLocation): string {
  return `${place.path}:${place.line}:${place.column}`;
}
