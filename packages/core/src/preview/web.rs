//! wasm-биндинги предпросмотра.
//!
//! Отдельный объект, а не метод `WebBackend`: предпросмотр не читает vault и не
//! владеет им, а держит только набор шрифтов. Отдельный объект заодно означает
//! отдельное заимствование на границе wasm-bindgen — сборка документа не
//! конфликтует с циклом опроса vault, который живёт в `WebBackend`.
//!
//! Здесь пока нет исполнителя в Web Worker (T022): сборка идёт в том же потоке,
//! то есть FR-007 выполняется лишь для коротких документов. Форма вызова от
//! появления воркера не изменится — изменится место исполнения.

use wasm_bindgen::prelude::*;

use super::cache::PackageCache;
use super::compile::{compile_note_resolved, CompiledDocument};
use super::fonts::FontSet;
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
}

#[wasm_bindgen]
impl PreviewEngine {
    /// Движок со встроенным набором шрифтов (FR-027).
    #[wasm_bindgen(constructor)]
    pub fn new() -> PreviewEngine {
        PreviewEngine {
            fonts: FontSet::builtin(),
            packages: PackageCache::new(),
        }
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
    ) -> Result<CompiledDocument, JsError> {
        compile_note_resolved(
            backend,
            &self.packages,
            note_path,
            source_text,
            revision,
            self.fonts.clone(),
        )
        .await
        .map_err(|err| JsError::new(&format!("{err:?}")))
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
    ) -> Result<CompiledDocument, JsError> {
        super::compile::compile_note_with_templates(
            backend,
            &self.packages,
            note_path,
            source_text,
            revision,
            self.fonts.clone(),
            &overrides.template_policy(),
        )
        .await
        .map_err(|err| JsError::new(&format!("{err:?}")))
    }
}

impl Default for PreviewEngine {
    fn default() -> Self {
        Self::new()
    }
}
