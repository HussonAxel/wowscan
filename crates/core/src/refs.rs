//! Lecture d'une liste d'objets collée par l'utilisateur.
//!
//! Formats acceptés, séparés par des espaces, virgules, points-virgules ou retours à la ligne :
//! - identifiant : `19019`
//! - lien Wowhead : `https://www.wowhead.com/fr/item=19019/tonnerre`
//! - lien en jeu : `|cffff8000|Hitem:19019::::::::80:::::|h[Tonnerre]|h|r`

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

static BRACKETED_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]").unwrap());
static ITEM_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"item[=:](\d+)").unwrap());

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRefs {
    /// Identifiants trouvés, sans doublon, dans l'ordre de saisie.
    pub item_ids: Vec<u32>,
    /// Morceaux de texte qui ne ressemblent à aucun format connu.
    pub unrecognized: Vec<String>,
}

pub fn parse_item_refs(input: &str) -> ParsedRefs {
    // Le nom entre crochets d'un lien en jeu peut contenir des espaces : on le retire d'abord.
    let input = BRACKETED_NAME.replace_all(input, " ");
    let mut parsed = ParsedRefs::default();

    for token in input.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let token = token.trim();
        if token.is_empty() || token == "|h|r" {
            continue;
        }
        let ids: Vec<u32> = if token.bytes().all(|b| b.is_ascii_digit()) {
            token.parse().into_iter().collect()
        } else {
            ITEM_REF.captures_iter(token).filter_map(|c| c[1].parse().ok()).collect()
        };
        if ids.is_empty() {
            parsed.unrecognized.push(token.to_owned());
        }
        for id in ids {
            if id > 0 && !parsed.item_ids.contains(&id) {
                parsed.item_ids.push(id);
            }
        }
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_formats_deduplicated() {
        let parsed = parse_item_refs(
            "19019, https://www.wowhead.com/fr/item=2589/étoffe-de-lin\n\
             |cffff8000|Hitem:19019::::::::80:::::|h[Tonnerre, lame bénie]|h|r;  \
             https://www.wowhead.com/item=12345?bonus=1 n'importe-quoi",
        );
        assert_eq!(parsed.item_ids, vec![19019, 2589, 12345]);
        assert_eq!(parsed.unrecognized, vec!["n'importe-quoi"]);
    }

    #[test]
    fn empty_input() {
        assert_eq!(parse_item_refs("  \n ,, "), ParsedRefs::default());
    }
}
