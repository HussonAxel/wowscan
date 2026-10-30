import { getVersion } from '@tauri-apps/api/app'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { useSyncExternalStore } from 'react'

import { isDemo } from '@/lib/api'

/**
 * Mises à jour de l'app : `latest.json` de la dernière release GitHub (voir release.yml),
 * installateur signé, vérifié avec la clé publique de tauri.conf.json.
 */
export interface UpdateState {
  status: 'idle' | 'checking' | 'upToDate' | 'available' | 'downloading' | 'error'
  /** Version installée (`null` en mode démo). */
  current: string | null
  update: Update | null
  downloaded: number
  /** Taille de l'installateur, si le serveur la donne. */
  size: number | null
  error: unknown
}

let state: UpdateState = { status: 'idle', current: null, update: null, downloaded: 0, size: null, error: null }
const listeners = new Set<() => void>()

function set(patch: Partial<UpdateState>) {
  state = { ...state, ...patch }
  listeners.forEach((l) => l())
}

if (!isDemo) void getVersion().then((current) => set({ current }))

export async function checkForUpdate(): Promise<Update | null> {
  if (isDemo || state.status === 'checking' || state.status === 'downloading') return state.update
  set({ status: 'checking', error: null })
  try {
    const update = await check()
    set({ status: update ? 'available' : 'upToDate', update })
    return update
  } catch (error) {
    set({ status: 'error', error })
    return null
  }
}

/** Télécharge et lance l'installateur ; sous Windows, il ferme l'app puis la relance. */
export async function installUpdate() {
  const { update } = state
  if (!update || state.status === 'downloading') return
  set({ status: 'downloading', downloaded: 0, size: null, error: null })
  try {
    await update.downloadAndInstall((event) => {
      if (event.event === 'Started') set({ size: event.data.contentLength ?? null })
      else if (event.event === 'Progress') set({ downloaded: state.downloaded + event.data.chunkLength })
    })
    await relaunch()
  } catch (error) {
    set({ status: 'error', error })
    throw error
  }
}

/** Avancement du téléchargement en %, `null` si la taille est inconnue. */
export function downloadPercent({ downloaded, size }: UpdateState): number | null {
  return size ? Math.min(100, Math.round((downloaded / size) * 100)) : null
}

const subscribe =(listener: () => void) => {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useUpdate(): UpdateState {
  return useSyncExternalStore(subscribe, () => state)
}
