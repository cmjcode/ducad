//! Sistem Internasionalisasi (i18n) untuk DUCAD menggunakan Fluent (`fluent-bundle`).
//!
//! Bahasa default aplikasi adalah **English (`en-US`)**, dengan dukungan penuh
//! untuk **Bahasa Indonesia (`id-ID`)** dan fallback otomatis ke English.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use serde::{Deserialize, Serialize};
use unic_langid::{langid, LanguageIdentifier};

/// Pilihan bahasa yang didukung oleh DUCAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    En,
    Id,
}

impl Language {
    /// Kode bahasa ISO / BCP-47.
    pub const fn code(&self) -> &'static str {
        match self {
            Language::En => "en-US",
            Language::Id => "id-ID",
        }
    }

    /// Label nama bahasa untuk UI dropdown/picker.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Id => "Bahasa Indonesia",
        }
    }

    /// Daftar semua bahasa yang didukung.
    pub const fn all() -> &'static [Language] {
        &[Language::En, Language::Id]
    }

    /// Konversi dari u8 index.
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Language::Id,
            _ => Language::En,
        }
    }

    /// Konversi ke u8 index.
    pub const fn to_u8(&self) -> u8 {
        match self {
            Language::En => 0,
            Language::Id => 1,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

static ACTIVE_LANGUAGE: AtomicU8 = AtomicU8::new(0);

/// Set bahasa aktif aplikasi secara global dan thread-safe.
pub fn set_language(lang: Language) {
    ACTIVE_LANGUAGE.store(lang.to_u8(), Ordering::SeqCst);
}

/// Dapatkan bahasa aktif aplikasi saat ini.
pub fn current_language() -> Language {
    Language::from_u8(ACTIVE_LANGUAGE.load(Ordering::SeqCst))
}

const EN_RESOURCE: &str = include_str!("../locales/en-US/main.ftl");
const ID_RESOURCE: &str = include_str!("../locales/id-ID/main.ftl");

struct LocaleStore {
    en_bundle: FluentBundle<FluentResource>,
    id_bundle: FluentBundle<FluentResource>,
}

static STORE: OnceLock<LocaleStore> = OnceLock::new();

fn get_store() -> &'static LocaleStore {
    STORE.get_or_init(|| {
        let en_lang: LanguageIdentifier = langid!("en-US");
        let mut en_bundle = FluentBundle::new_concurrent(vec![en_lang]);
        en_bundle.set_use_isolating(false);
        let en_res = FluentResource::try_new(EN_RESOURCE.to_string())
            .expect("Failed to parse English fluent resource");
        en_bundle
            .add_resource(en_res)
            .expect("Failed to add English resource to bundle");

        let id_lang: LanguageIdentifier = langid!("id-ID");
        let mut id_bundle = FluentBundle::new_concurrent(vec![id_lang]);
        id_bundle.set_use_isolating(false);
        let id_res = FluentResource::try_new(ID_RESOURCE.to_string())
            .expect("Failed to parse Indonesian fluent resource");
        id_bundle
            .add_resource(id_res)
            .expect("Failed to add Indonesian resource to bundle");

        LocaleStore {
            en_bundle,
            id_bundle,
        }
    })
}

/// Terjemahkan sebuah pesan kunci dengan bahasa tertentu.
pub fn translate_lang(lang: Language, key: &str, args: Option<&FluentArgs>) -> String {
    let store = get_store();
    let bundle = match lang {
        Language::En => &store.en_bundle,
        Language::Id => &store.id_bundle,
    };

    if let Some(msg) = bundle.get_message(key) {
        if let Some(pattern) = msg.value() {
            let mut errors = vec![];
            let value = bundle.format_pattern(pattern, args, &mut errors);
            if errors.is_empty() {
                return value.to_string();
            }
        }
    }

    // Fallback ke bahasa Inggris jika bukan English
    if lang != Language::En {
        if let Some(msg) = store.en_bundle.get_message(key) {
            if let Some(pattern) = msg.value() {
                let mut errors = vec![];
                let value = store.en_bundle.format_pattern(pattern, args, &mut errors);
                if errors.is_empty() {
                    return value.to_string();
                }
            }
        }
    }

    // Jika tidak ditemukan sama sekali di bundle mana pun, kembalikan key
    key.to_string()
}

/// Terjemahkan pesan berdasarkan bahasa yang sedang aktif.
pub fn translate(key: &str, args: Option<&FluentArgs>) -> String {
    translate_lang(current_language(), key, args)
}

pub use fluent_bundle;

/// Helper untuk membangun FluentArgs dari slice pasangan (&str, FluentValue).
pub fn make_args<'a, I>(iter: I) -> FluentArgs<'a>
where
    I: IntoIterator<Item = (&'a str, FluentValue<'a>)>,
{
    let mut args = FluentArgs::new();
    for (k, v) in iter {
        args.set(k, v);
    }
    args
}

/// Macro untuk mengambil string terjemahan berdasarkan bahasa aktif.
///
/// Contoh:
/// ```
/// use ducad_i18n::t;
/// let text = t!("tool-line");
/// let formatted = t!("topbar-unit", unit = "mm");
/// ```
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::translate($key, None)
    };
    ($key:expr, $($name:ident = $val:expr),* $(,)?) => {{
        let mut args = $crate::fluent_bundle::FluentArgs::new();
        $(
            args.set(stringify!($name), $val);
        )*
        $crate::translate($key, Some(&args))
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_syntax::ast::Entry;

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_default_language_is_english() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_language(Language::En);
        assert_eq!(current_language(), Language::En);
        let line_text = t!("tool-line");
        assert_eq!(line_text, "Line");
    }

    #[test]
    fn test_language_switch_to_indonesian() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_language(Language::Id);
        assert_eq!(current_language(), Language::Id);
        let line_text = t!("tool-line");
        assert_eq!(line_text, "Garis");

        // Switch back to English
        set_language(Language::En);
        assert_eq!(t!("tool-line"), "Line");
    }

    #[test]
    fn test_interpolation() {
        let _guard = TEST_LOCK.lock().unwrap();
        set_language(Language::En);
        let formatted = t!("topbar-unit", unit = "mm");
        assert_eq!(formatted, "Unit: mm");

        set_language(Language::Id);
        let formatted_id = t!("topbar-unit", unit = "mm");
        assert_eq!(formatted_id, "Satuan: mm");
    }

    #[test]
    fn test_key_parity() {
        let store = get_store();
        // Parse raw resources and check that all message IDs exist in both
        let en_res = FluentResource::try_new(EN_RESOURCE.to_string()).unwrap();
        let id_res = FluentResource::try_new(ID_RESOURCE.to_string()).unwrap();

        let mut en_keys = vec![];
        for entry in en_res.entries() {
            if let Entry::Message(msg) = entry {
                en_keys.push(msg.id.name.to_string());
            }
        }

        let mut id_keys = vec![];
        for entry in id_res.entries() {
            if let Entry::Message(msg) = entry {
                id_keys.push(msg.id.name.to_string());
            }
        }

        for k in &en_keys {
            assert!(
                store.id_bundle.has_message(k),
                "Indonesian bundle missing key: {}",
                k
            );
        }

        for k in &id_keys {
            assert!(
                store.en_bundle.has_message(k),
                "English bundle missing key: {}",
                k
            );
        }
    }
}
