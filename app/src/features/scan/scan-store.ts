import { useSyncExternalStore } from 'react'

import { api } from '@/lib/api'
import type { ScanResult } from '@/lib/types'

/**
 * État du scan, hors des composants : il continue et reste affiché
 * quand on passe sur une autre page puis qu'on revient.
 */
export interface ScanState {
  status: 'idle' | 'running' | 'done' | 'error'
  done: number
  total: number
  /** Marchés servis par le cache (aucun téléchargement). */
  cached: number
  result: ScanResult | null
  error: unknown
}

let state: ScanState = { status: 'idle', done: 0, total: 0, cached: 0, result: null, error: null }
const listeners = new Set<() => void>()

function set(patch: Partial<ScanState>) {
  state = { ...state, ...patch }
  listeners.forEach((l) => l())
}

export async function startScan(ids: number[]): Promise<ScanResult | null> {
  if (state.status === 'running' || !ids.length) return null
  set({ status: 'running', done: 0, total: 0, cached: 0, error: null })
  try {
    const result = await api.scan(ids, (event) => {
      if (event.type === 'started') set({ total: event.total })
      else set({ done: event.done, total: event.total, cached: state.cached + (event.type === 'sourceDone' && !event.downloaded ? 1 : 0) })
    })
    set({ status: 'done', result })
    return result
  } catch (error) {
    set({ status: 'error', error })
    throw error
  }
}

const subscribe = (listener: () => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useScan(): ScanState {
  return useSyncExternalStore(subscribe, () => state)
}
