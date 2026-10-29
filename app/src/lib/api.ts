import { Channel, invoke, isTauri } from '@tauri-apps/api/core'

import { mockApi } from './mock'
import type {
  CommandError,
  ConnectedRealm,
  ItemInfo,
  ParsedRefs,
  Plan,
  PlanInput,
  ScanEvent,
  ScanResult,
} from './types'

/** Appels au moteur Rust. Hors de Tauri (`pnpm dev` dans un navigateur), données de démo. */
export interface Api {
  hasCredentials(): Promise<boolean>
  saveCredentials(clientId: string, clientSecret: string): Promise<void>
  clearCredentials(): Promise<void>
  parseItems(input: string): Promise<ParsedRefs>
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
export const api: Api = isDemo ? mockApi : tauriApi

export function isCommandError(error: unknown): error is CommandError {
  return typeof error === 'object' && error !== null && 'code' in error && 'message' in error
}
