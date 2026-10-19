//! The catalogs hold what the default language holds.

use super::*;

#[test]
fn locale_keys_and_placeholders_match_default_language() {
    let reference = messages(DEFAULT);
    for locale in LOCALES.into_iter().filter(|locale| *locale != DEFAULT) {
        let current = messages(locale);
        assert_eq!(
            reference.keys().collect::<Vec<_>>(),
            current.keys().collect::<Vec<_>>(),
            "{locale} does not hold the keys the default language holds"
        );
        for key in reference.keys() {
            assert_eq!(
                placeholders(&reference[key]),
                placeholders(&current[key]),
                "placeholder mismatch for {key} in {locale}"
            );
        }
    }
}

/// The catalog that ships must agree with the file it was built from.
///
/// `rust_i18n::i18n!` resolves through a proc macro that reads the locale files while it
/// expands, so the compiler records no dependency on them: `dep-lib-bongocat_i18n` lists
/// `src/lib.rs` and nothing else. Freshness therefore rests entirely on `build.rs`'s
/// `rerun-if-changed` plus the revision it injects. Miss that path — a build that only
/// relinks the UI, a stale fingerprint, an editor that leaves the mtime behind — and the
/// crate keeps serving the previous copy while every other check stays green, because
/// `validate-locales.py` and both key scans above read the JSON rather than the catalog that
/// actually ships. The window then renders retired copy: on 2026-09-21 the model delete
/// confirmation displayed the pre-rename template `%{status} · %{confirm_deletion}` with
/// every gate passing.
#[test]
fn compiled_catalog_matches_the_files_on_disk() {
    for locale in LOCALES {
        for (key, expected) in messages_on_disk(locale) {
            assert_eq!(
                text(locale, &key),
                expected,
                "{locale}: `{key}` differs from locales/{locale}.json — this build is serving \
                 a stale catalog; rebuild with `cargo build -p bongocat-i18n`"
            );
        }
    }
}

#[test]
fn locale_source_uses_nested_snake_case_keys() {
    for locale in LOCALES {
        let value: serde_json::Value =
            serde_json::from_str(source(locale)).expect("valid locale JSON");
        fn visit(value: &serde_json::Value, path: &str) {
            if let Some(object) = value.as_object() {
                for (key, child) in object {
                    if key != "_version" {
                        assert!(!key.contains('.'), "flat key at {path}: {key}");
                        assert!(
                            key.chars()
                                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()),
                            "non-snake-case key at {path}: {key}"
                        );
                    }
                    visit(child, &format!("{path}.{key}"));
                }
            }
        }
        visit(&value, locale);
    }
}

#[test]
fn a_shipped_language_answers_in_its_own_catalog() {
    assert_eq!(text("zh-CN", "navigation.settings.title"), "BongoCat 设置");
    assert_eq!(
        text("en-US", "navigation.settings.title"),
        "BongoCat Settings"
    );
}

#[test]
fn a_tag_without_a_catalog_falls_back_to_english() {
    for locale in ["system", "de-DE", "ja-JP", "ru-RU", "ar-EG"] {
        assert_eq!(
            text(locale, "navigation.settings.title"),
            "BongoCat Settings",
            "{locale} is not a language this product ships"
        );
    }
}

/// Every region variant of a shipped language reaches that language's catalog.
///
/// The catalogs are named after one primary variety each — `en-US`, `zh-CN` —
/// so the codes a machine actually reports are almost never the shipped name
/// itself. This is the RFC 4647 language-subtag fallback that browsers' `Intl`,
/// CLDR and the operating system all perform, and it is what keeps a British,
/// Australian or bare-English machine and every Chinese tag on the same catalog
/// instead of silently dropping to English.
#[test]
fn a_region_variant_reaches_the_catalog_that_serves_its_language() {
    for (locale, shipped) in [
        ("en", "en-US"),
        ("en-GB", "en-US"),
        ("en_AU", "en-US"),
        ("EN-us", "en-US"),
        ("zh", "zh-CN"),
        ("zh-CN", "zh-CN"),
        ("zh_CN", "zh-CN"),
        ("zh-Hans", "zh-CN"),
        ("zh-Hans-CN", "zh-CN"),
        // One Chinese catalog means a Traditional tag reads Simplified copy:
        // the product ships one Chinese translation, not two.
        ("zh-TW", "zh-CN"),
        ("zh-HK", "zh-CN"),
        ("zh-Hant", "zh-CN"),
        ("zh_Hant_TW", "zh-CN"),
    ] {
        assert_eq!(
            locale_code(locale),
            shipped,
            "{locale} should resolve to the {shipped} catalog"
        );
        assert_eq!(
            text(locale, "navigation.settings.title"),
            text(shipped, "navigation.settings.title"),
            "{locale} rendered different copy from {shipped}"
        );
    }
}

/// The product ships exactly two catalogs, and both are reachable.
#[test]
fn the_shipped_catalog_list_matches_the_files() {
    assert_eq!(crate::SHIPPED_LOCALES, ["en-US", "zh-CN"]);
    for locale in crate::SHIPPED_LOCALES {
        assert_eq!(locale_code(locale), locale);
    }
}
