import { Channel, invoke, isTauri } from '@tauri-apps/api/core'

import type {
  CommandError,
  ConnectedRealm,
  ItemInfo,
  ItemRef,
  LocalizedString,
  ParsedRefs,
  Plan,
  PlanInput,
  ScanEvent,
  ScanResult,
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
