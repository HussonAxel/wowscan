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

/** Objet demandé ; ses bonus (lien Wowhead ou en jeu) désignent une version précise. */
export interface ItemRef {
  itemId: number
  bonusLists: number[]
}

export interface ParsedRefs {
  items: ItemRef[]
  /** Noms entre crochets sans lien (copie du chat sans les codes). */
  names: string[]
  battlePets: string[]
  appearances: string[]
  unrecognized: string[]
}

/** Version d'un objet déduite de ses bonus. */
export interface Variant {
  modifier: number
  appearanceId: number | null
  /** Identifiants de libellés (« Mythique »…), voir `variantLabels`. */
  labels: number[]
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

export interface VariantResult {
  appearanceId: number | null
  modifier: number
  labels: number[]
  /** Objet empilable : un seul prix pour toute la région. */
  commodity: PriceStats | null
  realms: RealmPrice[]
}

export interface ItemResult {
  itemId: number
  /** Versions en vente, de la moins chère à la plus chère. */
  variants: VariantResult[]
}

/** Un objet précis dans une version précise. */
export interface Source {
  itemId: number
  modifier: number
  appearanceId: number
}

export interface SetSummary {
  id: number
  name: LocalizedString
  label: number | null
  /** Bit n = classe n+1 ; 0 = toutes les classes. */
  classMask: number
  expansion: number
  appearances: number
}

/**
 * Origine d'un objet (`TransmogSourceTypeEnum` du jeu) : 1 boss, 2 quête, 3 marchand,
 * 4 butin du monde, 5 caché, 6 non collectable, 7 haut fait, 8 métier, 10 comptoir.
 */
export type SourceKind = number

/** Boss du journal des aventures qui laisse tomber un objet. */
export interface BossDrop {
  encounterId: number
  encounter: LocalizedString
  instanceId: number
  instance: LocalizedString
}

export interface PieceSource extends Source {
  kind: SourceKind
  drops: BossDrop[]
}

export interface SetPiece {
  appearanceId: number
  /** Emplacement (`InventoryType` du jeu : 1 tête, 3 épaules, 5 torse…). */
  slot: number | null
  setSources: Source[]
  allSources: PieceSource[]
}

export interface SetDetails extends SetSummary {
  pieces: SetPiece[]
}

export interface Character {
  name: string
  realm: string
  /** Identifiant de classe du jeu (1 Guerrier … 13 Évocateur). */
  classId: number
}

/** État de la collection : dossier du jeu, addon WowScan et dernier export lu. */
export interface CollectionStatus {
  gameDir: string | null
  addon: 'missing' | 'outdated' | 'installed' | null
  /** Date de l'analyse en jeu (secondes Unix). */
  scannedAt: number | null
  appearances: number
  characters: Character[]
}

/** Progression d'un ensemble, par emplacement comme dans le jeu. */
export interface SetCollection extends SetSummary {
  collected: number
  total: number
  /** Apparences des pièces manquantes. */
  missing: number[]
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
    | 'game_not_found'
    | 'game_folder'
  message: string
}
