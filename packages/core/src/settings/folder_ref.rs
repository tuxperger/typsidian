//! Реестр ссылок на папки vault (contracts/settings-store-trait.md → «Реестр
//! папок vault», research.md §10).
//!
//! **Ссылка на папку объявлена ассоциированным типом**, а не общим конкретным:
//! `PathBuf` на native, `FileSystemDirectoryHandle` на web. Форма API при этом
//! одна на обоих таргетах — вызывающий код получает ссылку от системного выбора
//! папки и передаёт её обратно, **не разбирая содержимого** и не ветвясь по
//! платформе.
//!
//! Причина, по которой тождество вообще спрятано за реестром: в браузере
//! абсолютного пути не существует. Сравнение по пути выродилось бы там в
//! сравнение по имени папки, и две разные папки `notes` схлопнулись бы в одну
//! запись — пользователь потерял бы доступ к одному из своих vault. Это не
//! отсутствие возможности, а неверное поведение.

use super::VaultId;

/// Чем закончилась попытка получить ссылку на папку по идентификатору.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved<R> {
    /// Ссылка получена и пригодна к использованию.
    Resolved(R),
    /// Требуется разрешение пользователя. **Не ошибка**: на платформах, где
    /// постоянный доступ к папке требует разрешения, оно переспрашивается по
    /// жесту пользователя, и отказ в разрешении тоже ошибкой не считается.
    NeedsPermission,
    /// Ссылки больше нет: папка удалена, перенесена так, что связь утрачена,
    /// или запись о ней потеряна.
    Missing,
}

impl<R> Resolved<R> {
    pub fn is_resolved(&self) -> bool {
        matches!(self, Resolved::Resolved(_))
    }

    /// Доступность записи для показа в списке (FR-023). `NeedsPermission`
    /// считается доступной: спросить разрешение — штатный путь, а не поломка.
    pub fn availability(&self) -> super::Availability {
        match self {
            Resolved::Resolved(_) | Resolved::NeedsPermission => super::Availability::Available,
            Resolved::Missing => super::Availability::Unavailable,
        }
    }
}

/// Реестр ссылок на папки vault.
///
/// Обязательства, общие для обеих реализаций:
///
/// 1. **Тождество устанавливает платформа.** `find_existing` сравнивает
///    канонизированные пути (native) либо вызывает `isSameEntry()` (web).
///    Сравнение имён и показываемых расположений **запрещено** (FR-020).
/// 2. **Никаких меток внутри vault.** Служебные файлы в папке заметок не
///    создаются ни при одной операции (FR-020, принцип VI).
/// 3. **Розыск запрещён.** Перенесённая или переименованная снаружи папка не
///    разыскивается по новому расположению ни на одной платформе; при утрате
///    связи запись становится недоступной (FR-023).
#[allow(async_fn_in_trait)]
pub trait FolderRegistry {
    /// Ссылка на папку: `PathBuf` на native, `FileSystemDirectoryHandle` на web.
    type FolderRef;

    /// Вернуть [`VaultId`], если кандидат — **та же самая папка**, что уже в
    /// списке.
    ///
    /// Асинхронна и сравнивает кандидата перебором по списку — до N
    /// асинхронных сравнений на одно добавление (`isSameEntry()` в вебе
    /// асинхронен). При рабочей границе списка в 50 записей это несущественно.
    async fn find_existing(&self, candidate: &Self::FolderRef) -> Option<VaultId>;

    /// Выдать новый [`VaultId`] и сохранить ссылку в реестре таргета.
    async fn add(
        &mut self,
        candidate: Self::FolderRef,
        location: String,
    ) -> Result<VaultId, super::store::StoreError>;

    /// Получить ссылку по идентификатору — три исхода, см. [`Resolved`].
    async fn resolve(&self, id: &VaultId) -> Resolved<Self::FolderRef>;

    /// Убрать ссылку из реестра, **не трогая папку на диске** (FR-024).
    async fn forget(&mut self, id: &VaultId) -> Result<(), super::store::StoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_permission_is_not_unavailable() {
        // Переспросить разрешение — штатный путь (contracts: «NeedsPermission
        // не является ошибкой»), поэтому запись остаётся доступной.
        let needs: Resolved<()> = Resolved::NeedsPermission;
        assert_eq!(needs.availability(), super::super::Availability::Available);
        assert!(!needs.is_resolved());

        let missing: Resolved<()> = Resolved::Missing;
        assert_eq!(
            missing.availability(),
            super::super::Availability::Unavailable
        );
    }
}
