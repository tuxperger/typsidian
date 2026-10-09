//! wasm-биндинги предпросмотра.
//!
//! Отдельный объект, а не метод `WebBackend`: предпросмотр не читает vault и не
//! владеет им, а держит только набор шрифтов. Отдельный объект заодно означает
//! отдельное заимствование на границе wasm-bindgen — сборка документа не
//! конфликтует с циклом опроса vault, который живёт в `WebBackend`.
//!
//! Синхронная фаза сборки идёт в Web Worker, когда интерфейс его подключил
//! (`setCompileWorker`, см. `worker.rs`); до этого и при его отказе — в этом
//! же потоке. Форма вызова от места исполнения не зависит.

use wasm_bindgen::prelude::*;

use super::anchor::ScrollAnchor;
use super::cache::PackageCache;
use super::compile::{compile_note_resolved, compile_note_resolved_on, CompiledDocument};
use super::fonts::FontSet;
use super::worker::WorkerExecutor;
use typst::foundations::Bytes;

/// Движок предпросмотра: набор шрифтов, кэш шаблонов и сборка документа.
#[wasm_bindgen]
pub struct PreviewEngine {
    fonts: FontSet,
    /// Файлы внешних шаблонов, пережившие прошлые сборки.
    ///
    /// Живёт в движке, а не в вызове: движок создаётся один раз на открытый
    /// vault, и именно эта длительность жизни делает кэш полезным. Без него
    /// заметка с большим шаблоном поднимала бы его заново на каждое нажатие
    /// клавиши — `cache.rs` объясняет, почему это сотня пересборок.
    packages: PackageCache,
    /// Сколько начертаний во встроенном наборе — всё, что сверх, пришло из
    /// vault и едет воркеру вместе со сборкой.
    builtin_fonts: usize,
    /// Исполнитель в Web Worker; `None` — сборка идёт на месте.
    executor: Option<WorkerExecutor>,
}

#[wasm_bindgen]
impl PreviewEngine {
    /// Движок со встроенным набором шрифтов (FR-027).
    #[wasm_bindgen(constructor)]
    pub fn new() -> PreviewEngine {
        let fonts = FontSet::builtin();
        PreviewEngine {
            builtin_fonts: fonts.len(),
            fonts,
            packages: PackageCache::new(),
            executor: None,
        }
    }

    /// Подключает сборку в Web Worker (FR-007). `post` — функция интерфейса
    /// `(сообщение) => Promise<ответ>`, владеющая воркером; её договор описан
    /// у [`WorkerExecutor`]. Экспорт в PDF по-прежнему идёт на месте: это
    /// явное действие с собственным ходом выполнения, а не фон набора текста.
    #[wasm_bindgen(js_name = setCompileWorker)]
    pub fn set_compile_worker(&mut self, post: js_sys::Function) {
        self.executor = Some(WorkerExecutor::new(post, self.builtin_fonts));
    }

    /// Сколько файлов шаблонов удержано между сборками. Нужен диагностике:
    /// по нему видно, работает ли кэш, без замеров времени.
    #[wasm_bindgen(js_name = cachedPackageFiles)]
    pub fn cached_package_files(&self) -> usize {
        self.packages.len()
    }

    /// Добавляет шрифты из папки vault (FR-027, FR-029).
    #[wasm_bindgen(js_name = addVaultFonts)]
    pub fn add_vault_fonts(&mut self, files: Vec<js_sys::Uint8Array>) {
        let bytes = files.into_iter().map(|file| Bytes::new(file.to_vec()));
        self.fonts = std::mem::take(&mut self.fonts).with_vault_fonts(bytes);
    }

    /// Сколько начертаний доступно документу. Нужен UI, чтобы отличить
    /// «шрифтов нет» от «документ пуст».
    #[wasm_bindgen(js_name = fontCount)]
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// Собирает заметку из текста редактора (FR-002, FR-006).
    ///
    /// `backend` нужен фазе резолва: связанные файлы — изображения, вставки
    /// досок, импорты — читаются через vault, своего доступа к диску у
    /// предпросмотра нет (принцип I). Движок по-прежнему не владеет vault:
    /// он получает его на время одного вызова.
    ///
    /// **Вызывать только через очередь `wasm-call-queue`**: пока идёт сборка,
    /// заимствование `WebBackend` занято, и параллельный вызов из цикла опроса
    /// упал бы «recursive use of an object».
    #[wasm_bindgen(js_name = compile)]
    pub async fn compile_js(
        &self,
        backend: &crate::vault::web::WebBackend,
        note_path: &str,
        source_text: &str,
        revision: u64,
        anchor: Option<ScrollAnchor>,
    ) -> Result<CompiledDocument, JsError> {
        let built = match &self.executor {
            Some(executor) => {
                compile_note_resolved_on(
                    executor,
                    backend,
                    &self.packages,
                    note_path,
                    source_text,
                    revision,
                    self.fonts.clone(),
                )
                .await
            }
            None => {
                compile_note_resolved(
                    backend,
                    &self.packages,
                    note_path,
                    source_text,
                    revision,
                    self.fonts.clone(),
                )
                .await
            }
        };
        built
            .map(|document| document.with_anchor(anchor.as_ref()))
            .map_err(|err| JsError::new(&format!("{err:?}")))
    }

    /// Отметка состояния связанных файлов (FR-020): изменилась — пора
    /// пересобрать. Содержимое файлов не читается. Через `wasm-call-queue`,
    /// как и сборка: вызов занимает заимствование `WebBackend`.
    #[wasm_bindgen(js_name = dependencyMark)]
    pub async fn dependency_mark_js(
        &self,
        backend: &crate::vault::web::WebBackend,
        paths: Vec<String>,
    ) -> String {
        super::compile::dependency_mark(backend, &paths).await
    }

    /// То же, но недостающие внешние шаблоны доустанавливаются (фича 007,
    /// FR-001).
    ///
    /// Отдельный вызов, а не флаг у `compile`: этот путь может **писать** в
    /// vault и обращаться в сеть, и разница обязана быть видна на границе, а не
    /// прятаться в параметре. Заметка без подключений проходит здесь ровно так
    /// же, как через `compile`, и в сеть не заходит (FR-008).
    ///
    /// Правила загрузки приходят из настроек уровня vault — «включена ли
    /// автозагрузка» и «какой адрес у этого шаблона» знает то место, которое их
    /// хранит, а не предпросмотр (принцип I).
    #[wasm_bindgen(js_name = compileWithTemplates)]
    pub async fn compile_with_templates_js(
        &self,
        backend: &mut crate::vault::web::WebBackend,
        note_path: &str,
        source_text: &str,
        revision: u64,
        overrides: crate::vault::vault_settings::VaultOverrides,
        anchor: Option<ScrollAnchor>,
    ) -> Result<CompiledDocument, JsError> {
        let policy = overrides.template_policy();
        let built = match &self.executor {
            Some(executor) => {
                super::compile::compile_note_with_templates_on(
                    executor,
                    backend,
                    &self.packages,
                    note_path,
                    source_text,
                    revision,
                    self.fonts.clone(),
                    &policy,
                )
                .await
            }
            None => {
                super::compile::compile_note_with_templates(
                    backend,
                    &self.packages,
                    note_path,
                    source_text,
                    revision,
                    self.fonts.clone(),
                    &policy,
                )
                .await
            }
        };
        built
            .map(|document| document.with_anchor(anchor.as_ref()))
            .map_err(|err| JsError::new(&format!("{err:?}")))
    }

    /// Экспортирует заметку в PDF (фича 009, contracts/core-export-api.md).
    ///
    /// Метод движка, а не свободная функция: экспорту нужны те же шрифты и тот
    /// же кэш шаблонов, что предпросмотру, — иначе заметка собиралась бы в
    /// одном месте и не собиралась в другом.
    ///
    /// `source_text` — текст редактора открытой заметки; `undefined` —
    /// читать с диска (пакет, FR-022). Вызывать через `wasm-call-queue`, как
    /// `compile`: сборка занимает заимствование `WebBackend`.
    #[wasm_bindgen(js_name = exportPdf)]
    pub async fn export_pdf_js(
        &self,
        backend: &mut crate::vault::web::WebBackend,
        note_path: &str,
        source_text: Option<String>,
        overrides: crate::vault::vault_settings::VaultOverrides,
    ) -> crate::export::ExportReport {
        crate::export::export_note_pdf(
            backend,
            &self.packages,
            note_path,
            source_text.as_deref(),
            self.fonts.clone(),
            &overrides,
        )
        .await
    }

    /// Установить шаблон по явному запросу пользователя (FR-010).
    ///
    /// Выключатель автозагрузки здесь не спрашивается намеренно: выключен
    /// **автоматический** поход в сеть, а не сама возможность получить шаблон.
    /// Адрес по-прежнему берётся из настроек папки.
    #[wasm_bindgen(js_name = installTemplate)]
    pub async fn install_template_js(
        &self,
        backend: &mut crate::vault::web::WebBackend,
        spec: &str,
        overrides: crate::vault::vault_settings::VaultOverrides,
    ) -> Result<super::packages::InstallReport, super::packages::InstallError> {
        let report =
            super::compile::install_template(backend, spec, &overrides.template_policy()).await?;
        // Ту же версию могли поставить поверх: прежние байты уходят из кэша,
        // а отметка о сверке отпечатка снимается — содержимое теперь другое.
        if let Some(parsed) = super::packages::parse_spec(spec) {
            self.packages.forget(&parsed);
            self.packages.forget_verified(&parsed);
        }
        Ok(report)
    }

    /// Установленные шаблоны — имя, версия, источник и отпечаток (US5 AS1).
    #[wasm_bindgen(js_name = installedTemplates)]
    pub async fn installed_templates_js(
        &self,
        backend: &crate::vault::web::WebBackend,
    ) -> Vec<super::packages::InstalledTemplate> {
        super::packages::installed_templates(backend).await
    }
}

impl Default for PreviewEngine {
    fn default() -> Self {
        Self::new()
    }
}
