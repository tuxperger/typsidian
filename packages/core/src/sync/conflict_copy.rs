//! Конфликтные копии Syncthing (фича 011, research §7).
//!
//! Syncthing, получив два изменения одного файла с разных устройств, оставляет
//! одно на месте, а второе кладёт рядом под именем
//! `<имя>.sync-conflict-<ГГГГММДД>-<ЧЧММСС>-<устройство>[.<расширение>]`, где
//! устройство — первые семь знаков его идентификатора (base32). Формат один
//! для всех версий Syncthing начиная с 0.14.

use serde::{Deserialize, Serialize};

const MARKER: &str = ".sync-conflict-";

/// Разобранное имя конфликтной копии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", derive(tsify_next::Tsify))]
#[cfg_attr(target_arch = "wasm32", tsify(into_wasm_abi, from_wasm_abi))]
pub struct ConflictCopyName {
    /// Имя исходного файла в той же папке.
    pub original: String,
    /// Время конфликта как его записал Syncthing: `ГГГГММДД-ЧЧММСС`, местное
    /// время устройства-источника.
    pub stamp: String,
    /// Короткий идентификатор устройства Syncthing, где правка проиграла.
    pub device: String,
}

/// Разбирает **имя файла** (без папки). `None` — это не конфликтная копия.
pub fn parse(file_name: &str) -> Option<ConflictCopyName> {
    // Ищется последнее вхождение: у копии копии маркер встречается дважды, и
    // исходным для неё является первая копия, а не исходная заметка.
    let at = file_name.rfind(MARKER)?;
    let stem = &file_name[..at];
    let rest = &file_name[at + MARKER.len()..];
    // `ГГГГММДД-ЧЧММСС-XXXXXXX` — 23 знака, дальше расширение или конец.
    if rest.len() < 23 {
        return None;
    }
    let (fixed, extension) = rest.split_at(23);
    let date = &fixed[..8];
    let time = &fixed[9..15];
    let device = &fixed[16..];
    let well_formed = fixed.as_bytes()[8] == b'-'
        && fixed.as_bytes()[15] == b'-'
        && date.bytes().all(|b| b.is_ascii_digit())
        && time.bytes().all(|b| b.is_ascii_digit())
        && device
            .bytes()
            .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b));
    if !well_formed || stem.is_empty() {
        return None;
    }
    if !(extension.is_empty() || extension.starts_with('.')) {
        return None;
    }
    Some(ConflictCopyName {
        original: format!("{stem}{extension}"),
        stamp: format!("{date}-{time}"),
        device: device.to_string(),
    })
}

/// Является ли имя файла конфликтной копией.
pub fn is_conflict_copy(file_name: &str) -> bool {
    parse(file_name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_copy_names_its_original() {
        let parsed = parse("Заметка.sync-conflict-20261003-120000-ABCDEFG.typ").unwrap();
        assert_eq!(parsed.original, "Заметка.typ");
        assert_eq!(parsed.stamp, "20261003-120000");
        assert_eq!(parsed.device, "ABCDEFG");
    }

    #[test]
    fn a_file_without_extension_is_understood() {
        let parsed = parse("README.sync-conflict-20261003-120000-ABC2345").unwrap();
        assert_eq!(parsed.original, "README");
    }

    #[test]
    fn a_copy_of_a_copy_points_at_the_first_copy() {
        let name =
            "a.sync-conflict-20260101-000000-AAAAAAA.sync-conflict-20260102-000000-BBBBBBB.typ";
        let parsed = parse(name).unwrap();
        assert_eq!(
            parsed.original,
            "a.sync-conflict-20260101-000000-AAAAAAA.typ"
        );
        assert!(is_conflict_copy(&parsed.original));
    }

    #[test]
    fn ordinary_names_are_not_copies() {
        for name in [
            "note.typ",
            "sync-conflict.typ",
            "a.sync-conflict-2026-ABCDEFG.typ",
            "a.sync-conflict-20261003-120000-abcdefg.typ",
            ".sync-conflict-20261003-120000-ABCDEFG.typ",
            "a.sync-conflict-20261003-120000-ABCDEFGx",
        ] {
            assert!(parse(name).is_none(), "{name}");
        }
    }
}
