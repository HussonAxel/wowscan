//! Lecture d'une liste d'objets collée par l'utilisateur.
//!
//! Formats reconnus :
//! - identifiant seul : `19019` (seulement si le texte est une simple liste, voir plus bas) ;
//! - lien Wowhead : `https://www.wowhead.com/fr/item=19019/tonnerre` ;
//! - lien en jeu brut : `|cnIQ4:|Hitem:19019::::::::80:::::|h[Tonnerre]|h|r`
//!   (anciennes couleurs `|cffa335ee` acceptées) ;
//! - nom entre crochets : `[Tonnerre]`, ce que donne un copier-coller du chat quand le jeu
//!   ou un add-on retire les codes des liens. Le nom est ensuite cherché via l'API
//!   ([`crate::search`]).
//!
//! Une ligne de chat contient aussi des crochets qui ne sont pas des objets : horodatage
//! `[23:15]`, canal `[2. Commerce]`, joueur `[Arthas]`. Les deux premiers sont écartés ici ;
//! un nom de joueur ne correspond simplement à aucun objet lors de la recherche.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Lien de jeu complet : `|H<type>:<données>|h[<texte>]|h`.
static HYPERLINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\|H([a-zA-Z]+):([^|]*)\|h\[([^\]]*)\]\|h").unwrap());
/// Codes de couleur et de mise en forme : `|cffRRGGBB`, `|cnIQ4:`, `|cn<NOM>:`, `|r`, icônes `|A…|a`.
static FORMATTING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\|c[0-9a-fA-F]{8}|\|cn[A-Za-z0-9_]+:|\|r|\|A[^|]*\|a|\|T[^|]*\|t").unwrap()
});
static BRACKETED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\[\]]+)\]").unwrap());
static ITEM_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"item[=:](\d+)").unwrap());
/// Bonus d'un lien Wowhead : `?bonus=9081` ou `&bonus=1799:1473`.
static WOWHEAD_BONUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[?&]bonus=([\d:]+)").unwrap());

/// Position du nombre de bonus dans les données d'un lien `item:` : identifiant,
/// enchantement, 4 gemmes, suffixe, unique, niveau, spécialisation, masque, contexte.
const LINK_BONUS_COUNT_FIELD: usize = 12;
/// Repère qui remplace un lien d'objet le temps de la lecture (caractère de contrôle,
/// absent de tout texte collé).
const LINK_MARK: char = '\u{1}';
/// Canal numéroté (`2. Commerce`) ou horodatage (`23:15`, `23:15:42`).
static NOT_AN_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+\.\s|\d{1,2}:\d{2}(:\d{2})?$)").unwrap());

/// Un objet demandé, avec ses bonus s'ils sont connus : ils désignent une version précise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRef {
    pub item_id: u32,
    pub bonus_lists: Vec<u32>,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRefs {
    /// Objets trouvés, sans doublon, dans l'ordre de saisie.
    pub items: Vec<ItemRef>,
    /// Noms entre crochets sans lien, à chercher par leur nom.
    pub names: Vec<String>,
    /// Liens de mascottes de combat : pas encore gérés par le scan.
    pub battle_pets: Vec<String>,
    /// Liens d'apparence (collection de transmogrification) : pas encore gérés.
    pub appearances: Vec<String>,
    /// Morceaux de liste qui ne ressemblent à aucun format connu (vide pour du texte de chat).
    pub unrecognized: Vec<String>,
}

impl ParsedRefs {
    fn push(&mut self, item_id: u32, bonus_lists: Vec<u32>) {
        let item = ItemRef { item_id, bonus_lists };
        if item_id > 0 && !self.items.contains(&item) {
            self.items.push(item);
        }
    }

    /// Identifiants seuls, sans doublon.
    pub fn item_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = Vec::new();
        for item in &self.items {
            if !ids.contains(&item.item_id) {
                ids.push(item.item_id);
            }
        }
        ids
    }

    fn push_unique(list: &mut Vec<String>, value: &str) {
        let value = value.trim();
        if !value.is_empty() && !list.iter().any(|v| v.eq_ignore_ascii_case(value)) {
            list.push(value.to_owned());
        }
    }
}

pub fn parse_item_refs(input: &str) -> ParsedRefs {
    let mut parsed = ParsedRefs::default();

    // 1. Liens complets, remplacés par un repère pour garder l'ordre de saisie.
    //    Les liens de joueur, de canal, de haut fait… sont ignorés.
    let mut links: Vec<(u32, Vec<u32>)> = Vec::new();
    let rest = HYPERLINK.replace_all(input, |link: &regex::Captures| {
        let text = FORMATTING.replace_all(&link[3], "");
        match &link[1] {
            "item" => {
                let fields: Vec<&str> = link[2].split(':').collect();
                if let Some(id) = fields.first().and_then(|id| id.parse().ok()) {
                    links.push((id, link_bonus_lists(&fields)));
                    return format!(" {LINK_MARK}{} ", links.len() - 1);
                }
            }
            "battlepet" => ParsedRefs::push_unique(&mut parsed.battle_pets, &text),
            "transmogappearance" | "transmogillusion" => ParsedRefs::push_unique(&mut parsed.appearances, &text),
            _ => {}
        }
        " ".to_owned()
    });
    let rest = FORMATTING.replace_all(&rest, "");

    // 2. Noms entre crochets restants.
    for name in BRACKETED.captures_iter(&rest) {
        let name = name[1].trim();
        if name.chars().any(char::is_alphabetic) && !NOT_AN_ITEM.is_match(name) {
            ParsedRefs::push_unique(&mut parsed.names, name);
        }
    }
    let rest = BRACKETED.replace_all(&rest, " ");

    // 3. Identifiants, liens Wowhead, `item:123`. Un nombre seul n'est pris pour un
    //    identifiant que si le texte est une liste : dans une phrase (« vends 500 po »),
    //    c'est un prix ou une quantité.
    let tokens: Vec<&str> = rest
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "|h")
        .collect();
    let is_list = tokens
        .iter()
        .all(|t| t.bytes().all(|b| b.is_ascii_digit()) || ITEM_REF.is_match(t) || t.starts_with(LINK_MARK));
    let is_chat = !links.is_empty() || !parsed.names.is_empty() || !is_list;

    for token in tokens {
        if let Some((id, bonus)) = token.strip_prefix(LINK_MARK).and_then(|i| i.parse::<usize>().ok()).and_then(|i| links.get(i)) {
            parsed.push(*id, bonus.clone());
            continue;
        }
        let refs: Vec<u32> = ITEM_REF.captures_iter(token).filter_map(|c| c[1].parse().ok()).collect();
        if !refs.is_empty() {
            let bonus: Vec<u32> = WOWHEAD_BONUS
                .captures(token)
                .map(|c| c[1].split(':').filter_map(|b| b.parse().ok()).collect())
                .unwrap_or_default();
            refs.into_iter().for_each(|id| parsed.push(id, bonus.clone()));
        } else if is_list && token.bytes().all(|b| b.is_ascii_digit()) {
            if let Ok(id) = token.parse() {
                parsed.push(id, Vec::new());
            }
        } else if !is_chat {
            parsed.unrecognized.push(token.to_owned());
        }
    }
    parsed
}

/// Bonus d'un lien en jeu : le champ 12 donne leur nombre, suivi des identifiants.
fn link_bonus_lists(fields: &[&str]) -> Vec<u32> {
    let count: usize = fields.get(LINK_BONUS_COUNT_FIELD).and_then(|c| c.parse().ok()).unwrap_or(0);
    fields
        .iter()
        .skip(LINK_BONUS_COUNT_FIELD + 1)
        .take(count)
        .filter_map(|b| b.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_list_of_ids_and_links() {
        let parsed = parse_item_refs(
            "19019, https://www.wowhead.com/fr/item=2589/étoffe-de-lin\n\
             https://www.wowhead.com/item=12345?bonus=1 19019",
        );
        assert_eq!(parsed.item_ids(), vec![19019, 2589, 12345]);
        assert_eq!(parsed.items[2].bonus_lists, vec![1]);
        assert!(parsed.names.is_empty() && parsed.unrecognized.is_empty());
    }

    #[test]
    fn raw_chat_line_with_modern_and_legacy_links() {
        let line = "[23:15] |Hchannel:CHANNEL:2|h[2. Commerce - Ville]|h \
            |Hplayer:Arthas-Hyjal:123:CHANNEL:2|h[Arthas]|h: VDS \
            |cnIQ4:|Hitem:178949::::::::80:::::|h[Claymore de Noir-Mâtin]|h|r et \
            |cffa335ee|Hitem:9480::::::::80:::::|h[Arrache-yeux]|h|r 500 po chacun, mp";
        let parsed = parse_item_refs(line);
        assert_eq!(parsed.item_ids(), vec![178949, 9480]);
        assert!(parsed.names.is_empty(), "timestamp, channel and player are not items: {:?}", parsed.names);
        assert!(parsed.unrecognized.is_empty(), "chat text is not reported");
    }

    #[test]
    fn versions_from_wowhead_and_game_links() {
        let parsed = parse_item_refs(
            "https://www.wowhead.com/item=124182/cord-of-unhinged-malice?bonus=9081 \
             https://www.wowhead.com/fr/item=124182?bonus=1799:1473&ilvl=720 \
             |cnIQ4:|Hitem:124182::::::::80:::5:1:9080:::::|h[Cord of Unhinged Malice]|h|r \
             https://www.wowhead.com/item=124182",
        );
        let bonus: Vec<Vec<u32>> = parsed.items.iter().map(|i| i.bonus_lists.clone()).collect();
        assert_eq!(bonus, vec![vec![9081], vec![1799, 1473], vec![9080], vec![]]);
        assert_eq!(parsed.item_ids(), vec![124182], "four versions of a single item");
    }

    #[test]
    fn crafted_quality_icon_inside_link_name() {
        let parsed = parse_item_refs(
            "|cnIQ3:|Hitem:211296::::::::80::::2:10255:6652:::::|h[Fer de lance |A:Professions-ChatIcon-Quality-Tier3:17:17::1|a]|h|r",
        );
        assert_eq!(parsed.items, vec![ItemRef { item_id: 211296, bonus_lists: vec![10255, 6652] }]);
    }

    #[test]
    fn stripped_chat_line_gives_names() {
        let parsed = parse_item_refs(
            "[23:15] [2. Commerce] [Arthas]: VDS [Claymore de Noir-Mâtin] [Arrache-yeux] 500 po",
        );
        assert!(parsed.items.is_empty(), "500 is a price, not an item");
        assert_eq!(parsed.names, vec!["Arthas", "Claymore de Noir-Mâtin", "Arrache-yeux"]);
    }

    #[test]
    fn pets_and_appearances_are_reported_separately() {
        let parsed = parse_item_refs(
            "|cff0070dd|Hbattlepet:1387:25:3:1546:276:276:0000000000000000|h[Petit tonnerre]|h|r \
             |cffff80ff|Htransmogappearance:12345|h[Heaume de l'aube]|h|r",
        );
        assert_eq!(parsed.battle_pets, vec!["Petit tonnerre"]);
        assert_eq!(parsed.appearances, vec!["Heaume de l'aube"]);
        assert!(parsed.items.is_empty());
    }

    #[test]
    fn unknown_tokens_only_reported_for_lists() {
        let parsed = parse_item_refs("19019 n'importe-quoi");
        assert!(parsed.items.is_empty(), "mixed text is treated as chat");
        assert!(parsed.unrecognized.is_empty());

        assert_eq!(parse_item_refs("  \n ,, "), ParsedRefs::default());
    }
}
