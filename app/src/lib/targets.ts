import type { ItemRef, RealmPrice, ScanResult, VariantResult } from './types'

/**
 * Ce qu'on cherche à acheter :
 * - un objet, éventuellement dans une version précise (bonus d'un lien) ;
 * - une apparence (pièce d'ensemble), que plusieurs objets peuvent donner.
 */
export type Target =
  | ({ kind: 'item' } & ItemRef)
  | {
      kind: 'appearance'
      appearanceId: number
      /** Objets (toutes versions) qui donnent cette apparence. */
      itemIds: number[]
      /** Objet représentatif, pour le nom et l'icône. */
      itemId: number
      setId?: number
    }

export const targetKey = (t: Target) =>
  t.kind === 'item' ? `i:${t.itemId}:${t.bonusLists.join('.')}` : `a:${t.appearanceId}`

export const targetItemIds = (t: Target) => (t.kind === 'item' ? [t.itemId] : t.itemIds)

/** Prix d'un royaume, avec l'objet à acheter (utile quand plusieurs objets donnent la même apparence). */
export interface OptionRealm extends RealmPrice {
  itemId: number
}

/** Une option d'achat pour une cible : une version d'un objet, ou une apparence toutes sources confondues. */
export interface TargetOption {
  key: string
  appearanceId: number | null
  labels: number[]
  commodity: VariantResult['commodity']
  realms: OptionRealm[]
  best: number | null
}

function fromVariant(itemId: number, v: VariantResult): TargetOption {
  return {
    key: `${v.appearanceId ?? `m${v.modifier}`}`,
    appearanceId: v.appearanceId,
    labels: v.labels,
    commodity: v.commodity,
    realms: v.realms.map((r) => ({ ...r, itemId })),
    best: v.commodity?.min ?? v.realms[0]?.stats.min ?? null,
  }
}

/** Options d'achat d'une cible dans un résultat de scan, de la moins chère à la plus chère. */
export function targetOptions(target: Target, result: ScanResult): TargetOption[] {
  if (target.kind === 'item') {
    const item = result.items.find((i) => i.itemId === target.itemId)
    return item?.variants.map((v) => fromVariant(item.itemId, v)) ?? []
  }
  // Apparence : on fusionne toutes les versions qui la donnent, au prix le plus bas par royaume.
  const byRealm = new Map<number, OptionRealm>()
  for (const item of result.items.filter((i) => target.itemIds.includes(i.itemId))) {
    for (const variant of item.variants.filter((v) => v.appearanceId === target.appearanceId)) {
      for (const realm of variant.realms) {
        const known = byRealm.get(realm.connectedRealmId)
        if (!known || realm.stats.min < known.stats.min) byRealm.set(realm.connectedRealmId, { ...realm, itemId: item.itemId })
      }
    }
  }
  const realms = [...byRealm.values()].sort((a, b) => a.stats.min - b.stats.min)
  if (!realms.length) return []
  return [{ key: `${target.appearanceId}`, appearanceId: target.appearanceId, labels: [], commodity: null, realms, best: realms[0].stats.min }]
}
