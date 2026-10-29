//! Liste d'achat : sur quels royaumes acheter pour payer le moins cher.
//!
//! Acheter sur un royaume où l'on n'a pas de personnage a un coût (créer un
//! personnage, s'y rendre, ramener l'objet par la banque de bataillon). On le
//! modélise par un prix fixe par nouveau royaume, en or : c'est un problème de
//! « localisation d'entrepôts » classique. On part de la solution la moins chère
//! objet par objet, puis on ajoute, retire ou échange des royaumes tant que le
//! total baisse. Avec quelques dizaines d'objets et ~90 royaumes, c'est instantané.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::Money;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub connected_realm_id: u32,
    pub price: Money,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Want {
    pub item_id: u32,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInput {
    pub wants: Vec<Want>,
    /// Groupes de royaumes où l'on a déjà un personnage : aucun coût d'accès.
    pub owned_realms: HashSet<u32>,
    /// Coût estimé d'un royaume où l'on n'a pas encore de personnage.
    pub new_realm_cost: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Purchase {
    pub item_id: u32,
    pub connected_realm_id: u32,
    pub price: Money,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub purchases: Vec<Purchase>,
    /// Royaumes à visiter, déjà possédés ou non.
    pub realms: Vec<u32>,
    /// Parmi eux, ceux où il faut un nouveau personnage.
    pub new_realms: Vec<u32>,
    pub items_cost: Money,
    pub new_realms_cost: Money,
    pub total: Money,
    /// Objets introuvables à l'hôtel des ventes.
    pub unavailable: Vec<u32>,
}

struct Problem<'a> {
    /// Pour chaque objet disponible : prix par royaume.
    prices: Vec<(u32, HashMap<u32, u64>)>,
    owned: &'a HashSet<u32>,
    new_realm_cost: u64,
}

impl Problem<'_> {
    /// Coût total d'un ensemble de royaumes, `None` si un objet n'y est pas trouvable.
    fn cost(&self, open: &BTreeSet<u32>) -> Option<u64> {
        let mut total = 0u64;
        for (_, prices) in &self.prices {
            let best = open.iter().filter_map(|r| prices.get(r)).min()?;
            total = total.saturating_add(*best);
        }
        let new = open.iter().filter(|r| !self.owned.contains(r)).count() as u64;
        Some(total.saturating_add(new.saturating_mul(self.new_realm_cost)))
    }

    fn all_realms(&self) -> BTreeSet<u32> {
        self.prices.iter().flat_map(|(_, p)| p.keys().copied()).collect()
    }
}

pub fn plan(input: &PlanInput) -> Plan {
    let mut unavailable = Vec::new();
    let mut prices = Vec::new();
    for want in &input.wants {
        let mut best: HashMap<u32, u64> = HashMap::new();
        for c in &want.candidates {
            let entry = best.entry(c.connected_realm_id).or_insert(c.price.0);
            *entry = (*entry).min(c.price.0);
        }
        if best.is_empty() {
            unavailable.push(want.item_id);
        } else {
            prices.push((want.item_id, best));
        }
    }
    let problem = Problem { prices, owned: &input.owned_realms, new_realm_cost: input.new_realm_cost.0 };

    // La recherche locale peut rester bloquée dans un optimum local : on la lance
    // depuis deux points de départ et on garde le meilleur résultat.
    let open = [cheapest_per_item(&problem), greedy(&problem)]
        .into_iter()
        .map(|start| local_search(&problem, start))
        .min_by_key(|(cost, _)| *cost)
        .map(|(_, open)| open)
        .unwrap_or_default();

    let purchases: Vec<Purchase> = problem
        .prices
        .iter()
        .filter_map(|(item_id, prices)| {
            // À prix égal, on préfère un royaume où l'on a déjà un personnage.
            open.iter()
                .filter_map(|r| prices.get(r).map(|p| (*p, !problem.owned.contains(r), *r)))
                .min()
                .map(|(price, _, realm)| Purchase { item_id: *item_id, connected_realm_id: realm, price: Money(price) })
        })
        .collect();

    // Un royaume ouvert sans achat (possible après un échange) n'a rien à faire dans le plan.
    let used: BTreeSet<u32> = purchases.iter().map(|p| p.connected_realm_id).collect();
    let new_realms: Vec<u32> = used.iter().copied().filter(|r| !problem.owned.contains(r)).collect();
    let items_cost = Money(purchases.iter().map(|p| p.price.0).sum());
    let new_realms_cost = Money(new_realms.len() as u64 * problem.new_realm_cost);

    Plan {
        purchases,
        realms: used.into_iter().collect(),
        new_realms,
        items_cost,
        new_realms_cost,
        total: Money(items_cost.0 + new_realms_cost.0),
        unavailable,
    }
}

/// Ajoute, retire ou échange un royaume tant que le total baisse.
fn local_search(problem: &Problem, mut open: BTreeSet<u32>) -> (u64, BTreeSet<u32>) {
    let mut cost = problem.cost(&open).unwrap_or(u64::MAX);
    let all = problem.all_realms();
    loop {
        let mut best: Option<(u64, BTreeSet<u32>)> = None;
        let mut consider = |set: BTreeSet<u32>| {
            if let Some(c) = problem.cost(&set)
                && c < best.as_ref().map_or(cost, |(b, _)| *b)
            {
                best = Some((c, set));
            }
        };
        for &r in &open {
            let mut without = open.clone();
            without.remove(&r);
            for &add in all.difference(&open) {
                let mut swapped = without.clone();
                swapped.insert(add);
                consider(swapped);
            }
            consider(without);
        }
        for &add in all.difference(&open) {
            let mut with = open.clone();
            with.insert(add);
            consider(with);
        }
        match best {
            Some((c, set)) => (cost, open) = (c, set),
            None => return (cost, open),
        }
    }
}

/// Point de départ « constructif » : on part des royaumes possédés, puis on ajoute
/// à chaque tour le royaume qui fait le plus baisser le total. Un objet pas encore
/// couvert compte pour son prix maximal plus le coût d'un royaume, pour que couvrir
/// un objet soit toujours rentable.
fn greedy(problem: &Problem) -> BTreeSet<u32> {
    let penalty = |prices: &HashMap<u32, u64>| {
        prices.values().max().copied().unwrap_or(0).saturating_add(problem.new_realm_cost).saturating_add(1)
    };
    let penalized = |open: &BTreeSet<u32>| -> u64 {
        let items: u64 = problem
            .prices
            .iter()
            .map(|(_, p)| open.iter().filter_map(|r| p.get(r)).min().copied().unwrap_or_else(|| penalty(p)))
            .fold(0, u64::saturating_add);
        let new = open.iter().filter(|r| !problem.owned.contains(r)).count() as u64;
        items.saturating_add(new.saturating_mul(problem.new_realm_cost))
    };

    let all = problem.all_realms();
    let mut open: BTreeSet<u32> = all.iter().copied().filter(|r| problem.owned.contains(r)).collect();
    let mut cost = penalized(&open);
    loop {
        let best = all
            .difference(&open)
            .map(|&r| {
                let mut with = open.clone();
                with.insert(r);
                (penalized(&with), with)
            })
            .min_by_key(|(c, _)| *c);
        match best {
            Some((c, with)) if c < cost => (cost, open) = (c, with),
            _ => return open,
        }
    }
}

/// Point de départ : le royaume le moins cher de chaque objet.
fn cheapest_per_item(problem: &Problem) -> BTreeSet<u32> {
    problem
        .prices
        .iter()
        .filter_map(|(_, prices)| {
            prices
                .iter()
                .map(|(r, p)| (*p, !problem.owned.contains(r), *r))
                .min()
                .map(|(_, _, r)| r)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn want(item_id: u32, offers: &[(u32, u64)]) -> Want {
        Want {
            item_id,
            candidates: offers
                .iter()
                .map(|&(r, g)| Candidate { connected_realm_id: r, price: Money::from_gold(g) })
                .collect(),
        }
    }

    fn input(wants: Vec<Want>, owned: &[u32], new_realm_gold: u64) -> PlanInput {
        PlanInput {
            wants,
            owned_realms: owned.iter().copied().collect(),
            new_realm_cost: Money::from_gold(new_realm_gold),
        }
    }

    #[test]
    fn free_new_realms_means_cheapest_everywhere() {
        let plan = plan(&input(vec![want(1, &[(10, 100), (20, 50)]), want(2, &[(10, 30), (30, 20)])], &[], 0));
        assert_eq!(plan.realms, vec![20, 30]);
        assert_eq!(plan.total, Money::from_gold(70));
    }

    #[test]
    fn owned_realm_wins_when_new_realm_costs_more_than_savings() {
        // Royaume 20 : 40 po moins cher, mais y aller coûte 100 po.
        let plan = plan(&input(vec![want(1, &[(10, 140), (20, 100)])], &[10], 100));
        assert_eq!(plan.realms, vec![10]);
        assert!(plan.new_realms.is_empty());
        assert_eq!(plan.total, Money::from_gold(140));
    }

    #[test]
    fn groups_purchases_on_one_new_realm() {
        // Chaque objet est le moins cher sur un royaume différent, mais le royaume 30
        // a les trois à peine plus cher : un seul nouveau personnage suffit.
        let wants = vec![
            want(1, &[(10, 100), (30, 110)]),
            want(2, &[(20, 100), (30, 110)]),
            want(3, &[(40, 100), (30, 110)]),
        ];
        let plan = plan(&input(wants, &[], 200));
        assert_eq!(plan.realms, vec![30]);
        assert_eq!(plan.total, Money::from_gold(330 + 200));
    }

    #[test]
    fn unavailable_items_are_reported() {
        let plan = plan(&input(vec![want(1, &[(10, 5)]), want(2, &[])], &[], 0));
        assert_eq!(plan.unavailable, vec![2]);
        assert_eq!(plan.purchases.len(), 1);
    }

    #[test]
    fn equal_price_prefers_owned_realm() {
        let plan = plan(&input(vec![want(1, &[(10, 50), (20, 50)])], &[20], 0));
        assert_eq!(plan.purchases[0].connected_realm_id, 20);
        assert!(plan.new_realms.is_empty());
    }
}
