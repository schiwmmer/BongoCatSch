//! What the user sees.
//!
//! The language is resolved rather than stored as a bare string: the document
//! records what the user picked, and `from_system_locale` is what turns an unset
//! or unusable value into the language the operating system actually reports.

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(any(test, feature = "schema-generation"), derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AppearanceConfig {
    pub theme: Theme,
    pub language: Language,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "schema-generation"), derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(any(test, feature = "schema-generation"), derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    #[default]
    System,
    // Simplified Chinese is the one Chinese catalog the product ships. A
    // Traditional tag reaches it too: the alternative is English, and a
    // Traditional reader is better served by Simplified copy than by none.
    #[serde(rename = "zh-CN")]
    ChineseSimplified,
    #[serde(rename = "en-US")]
    English,
}

impl Language {
    /// Every choice the language selector offers, in catalog order.
    pub const ALL: [Self; 3] = [Self::System, Self::ChineseSimplified, Self::English];

    pub const fn code(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::ChineseSimplified => "zh-CN",
            Self::English => "en-US",
        }
    }

    /// Classify the locale the operating system reports onto a shipped language.
    ///
    /// The product ships two catalogs, so the RFC 4647 primary-subtag fallback
    /// only has to decide between them: every `zh` tag — Simplified or
    /// Traditional, any region — reaches the one Chinese catalog, and every
    /// other language reaches English.
    pub fn from_system_locale(locale: &str) -> Self {
        let locale = locale.replace('_', "-").to_ascii_lowercase();
        match locale.split('-').next() {
            Some("zh") => Self::ChineseSimplified,
            _ => Self::English,
        }
    }

    pub const fn resolve(self, system_language: Self) -> Self {
        match self {
            Self::System => match system_language {
                Self::ChineseSimplified => Self::ChineseSimplified,
                Self::System | Self::English => Self::English,
            },
            Self::ChineseSimplified => Self::ChineseSimplified,
            Self::English => Self::English,
        }
    }
}
