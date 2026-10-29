// Miroir des types sérialisés par `wowscan-core` (voir crates/core/src).

/** Texte traduit : `{ fr_FR: "…", en_GB: "…" }`. */
export type LocalizedString = Record<string, string>

/** Montant en pièces de cuivre. */
export type Copper = number

export interface Realm {
  id: number
  name: LocalizedString
  slug: string
}

export interface ConnectedRealm {
  id: number
  realms: Realm[]
}

export type Quality =
  | 'POOR'
  | 'COMMON'
  | 'UNCOMMON'
  | 'RARE'
  | 'EPIC'
  | 'LEGENDARY'
  | 'ARTIFACT'
  | 'HEIRLOOM'

export interface ItemInfo {
  id: number
  name: LocalizedString
  quality: Quality | string
  itemClass: LocalizedString
  itemSubclass: LocalizedString
  inventoryType: LocalizedString
  icon: string | null
}

export interface ParsedRefs {
  itemIds: number[]
  unrecognized: string[]
}

export interface PriceStats {
  min: Copper
  auctions: number
  quantity: number
}

export interface RealmPrice {
  connectedRealmId: number
  stats: PriceStats
}

export interface ItemResult {
  itemId: number
  /** Objet empilable : un seul prix pour toute la région. */
  commodity: PriceStats | null
  realms: RealmPrice[]
}

export interface ScanResult {
  realms: ConnectedRealm[]
  items: ItemResult[]
  failures: { realmId: number | null; error: string }[]
  scannedAt: number
}

export type ScanEvent =
  | { type: 'started'; total: number }
  | { type: 'sourceDone'; realmId: number | null; downloaded: boolean; done: number; total: number }
  | { type: 'sourceFailed'; realmId: number | null; error: string; done: number; total: number }

export interface PlanInput {
  wants: { itemId: number; candidates: { connectedRealmId: number; price: Copper }[] }[]
  ownedRealms: number[]
  newRealmCost: Copper
}

export interface Plan {
  purchases: { itemId: number; connectedRealmId: number; price: Copper }[]
  realms: number[]
  newRealms: number[]
  itemsCost: Copper
  newRealmsCost: Copper
  total: Copper
  unavailable: number[]
}

/** Erreur renvoyée par une commande Tauri. */
export interface CommandError {
  code:
    | 'invalid_credentials'
    | 'no_credentials'
    | 'network'
    | 'rate_limited'
    | 'api'
    | 'bad_response'
    | 'cache'
    | 'keyring'
  message: string
}
