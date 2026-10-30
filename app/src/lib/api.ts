import { Channel, invoke, isTauri } from '@tauri-apps/api/core'

import type {
  CollectionStatus,
  CommandError,
  ConnectedRealm,
  ItemInfo,
  ItemRef,
  LocalizedString,
  MountCollection,
  MountTooltip,
  ParsedRefs,
  Plan,
  PlanInput,
  ScanEvent,
  ScanResult,
  SetCollection,
  SetDetails,
  SetSummary,
  Variant,
} from './types'

/** Appels au moteur Rust. Hors de Tauri (`pnpm dev` dans un navigateur), données de démo. */
export interface Api {
  hasCredentials(): Promise<boolean>
  saveCredentials(clientId: string, clientSecret: string): Promise<void>
  clearCredentials(): Promise<void>
  parseItems(input: string): Promise<ParsedRefs>
  /** Objets dont le nom correspond exactement (pour les copies du chat sans liens). */
  findItemsByName(names: string[]): Promise<Record<string, number[]>>
  resolveVariants(items: ItemRef[]): Promise<Variant[]>
  variantLabels(): Promise<Record<number, LocalizedString>>
  searchSets(query: string): Promise<SetSummary[]>
  setDetails(id: number): Promise<SetDetails | null>
  /** Objets qui donnent chacune de ces apparences. */
  appearanceItems(ids: number[]): Promise<Record<number, number[]>>
  /** Dossier du jeu (deviné si `gameDir` est vide), addon et dernier export. */
  collectionStatus(gameDir: string | null): Promise<CollectionStatus>
  installAddon(gameDir: string | null): Promise<CollectionStatus>
  /** Progression de chaque ensemble d'après l'export de l'addon. */
  collectionSets(gameDir: string | null): Promise<SetCollection[]>
  /** Montures du jeu et celles collectées d'après l'export de l'addon. */
  collectionMounts(gameDir: string | null): Promise<MountCollection>
  mountTooltip(mountId: number, itemId: number | null): Promise<MountTooltip>
  connectedRealms(): Promise<ConnectedRealm[]>
  itemInfos(ids: number[]): Promise<Record<number, ItemInfo>>
  scan(ids: number[], onEvent: (event: ScanEvent) => void): Promise<ScanResult>
  plan(input: PlanInput): Promise<Plan>
}

const tauriApi: Api = {
  hasCredentials: () => invoke('has_credentials'),
  saveCredentials: (clientId, clientSecret) => invoke('save_credentials', { clientId, clientSecret }),
  clearCredentials: () => invoke('clear_credentials'),
  parseItems: (input) => invoke('parse_items', { input }),
  findItemsByName: (names) => invoke('find_items_by_name', { names }),
  resolveVariants: (items) => invoke('resolve_variants', { items }),
  variantLabels: () => invoke('variant_labels'),
  searchSets: (query) => invoke('search_sets', { query }),
  setDetails: (id) => invoke('set_details', { id }),
  appearanceItems: (ids) => invoke('appearance_items', { ids }),
  collectionStatus: (gameDir) => invoke('collection_status', { gameDir }),
  installAddon: (gameDir) => invoke('install_addon', { gameDir }),
  collectionSets: (gameDir) => invoke('collection_sets', { gameDir }),
  collectionMounts: (gameDir) => invoke('collection_mounts', { gameDir }),
  mountTooltip: (mountId, itemId) => invoke('mount_tooltip', { mountId, itemId }),
  connectedRealms: () => invoke('connected_realms'),
  itemInfos: (ids) => invoke('item_infos', { ids }),
  scan: (ids, onEvent) => {
    const channel = new Channel<ScanEvent>()
    channel.onmessage = onEvent
    return invoke('scan_items', { ids, onEvent: channel })
  },
  plan: (input) => invoke('plan_purchases', { input }),
}

export const isDemo = !isTauri()

/** En mode démo, chaque appel passe par les données de démo, chargées au premier usage. */
function lazyMock(): Api {
  const load = () => import('./mock').then((m) => m.mockApi)
  return new Proxy({} as Api, {
    get: (_, method: keyof Api) =>
      (...args: unknown[]) =>
        load().then((mock) => (mock[method] as (...a: unknown[]) => unknown)(...args)),
  })
}

export const api: Api = isDemo ? lazyMock() : tauriApi

export function isCommandError(error: unknown): error is CommandError {
  return typeof error === 'object' && error !== null && 'code' in error && 'message' in error
}
