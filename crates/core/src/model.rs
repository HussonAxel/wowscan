use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Texte traduit, tel que renvoyé par l'API quand on ne précise pas de `locale`
/// (`{"fr_FR": "...", "en_GB": "...", ...}`). Accepte aussi une chaîne simple.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawLocalized")]
pub struct LocalizedString(pub BTreeMap<String, String>);

#[derive(Deserialize)]
#[serde(untagged)]
enum RawLocalized {
    Map(BTreeMap<String, Option<String>>),
    Plain(String),
}

impl From<RawLocalized> for LocalizedString {
    fn from(raw: RawLocalized) -> Self {
        match raw {
            RawLocalized::Map(map) => {
                Self(map.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).collect())
            }
            RawLocalized::Plain(s) => Self(BTreeMap::from([("en_GB".to_owned(), s)])),
        }
    }
}

impl LocalizedString {
    /// Texte dans la langue demandée (`fr_FR`, `en_GB`…), avec repli sur l'anglais.
    pub fn get(&self, locale: &str) -> &str {
        [locale, "en_GB", "en_US"]
            .iter()
            .find_map(|l| self.0.get(*l))
            .or_else(|| self.0.values().next())
            .map_or("", String::as_str)
    }
}

/// Montant en pièces de cuivre (1 po = 100 pa = 10 000 pc).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(pub u64);

impl Money {
    pub const fn from_gold(gold: u64) -> Self {
        Self(gold * 10_000)
    }

    pub const fn gold(self) -> u64 {
        self.0 / 10_000
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (g, s, c) = (self.0 / 10_000, self.0 / 100 % 100, self.0 % 100);
        if g > 0 {
            write!(f, "{g}g {s:02}s {c:02}c")
        } else if s > 0 {
            write!(f, "{s}s {c:02}c")
        } else {
            write!(f, "{c}c")
        }
    }
}

/// Lien `{"href": "..."}` utilisé partout dans l'API.
#[derive(Debug, Clone, Deserialize)]
pub struct Href {
    pub href: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_map_and_plain() {
        let map: LocalizedString =
            serde_json::from_str(r#"{"fr_FR":"Épée","en_GB":"Sword","de_DE":null}"#).unwrap();
        assert_eq!(map.get("fr_FR"), "Épée");
        assert_eq!(map.get("es_ES"), "Sword");

        let plain: LocalizedString = serde_json::from_str(r#""Sword""#).unwrap();
        assert_eq!(plain.get("fr_FR"), "Sword");
    }

    #[test]
    fn money_display() {
        assert_eq!(Money(1_234_567).to_string(), "123g 45s 67c");
        assert_eq!(Money(4_501).to_string(), "45s 01c");
        assert_eq!(Money(7).to_string(), "7c");
        assert_eq!(Money::from_gold(5).gold(), 5);
    }
}
