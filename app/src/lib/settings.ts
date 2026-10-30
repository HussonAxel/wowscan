import { useSyncExternalStore } from 'react'

import type { Language } from '@/i18n'

import type { Target } from './targets'

/** Préférences locales. Les identifiants Blizzard n'y sont pas : ils vont dans le coffre du système. */
export type PreferredVersion = 'cheapest' | 'mythic' | 'heroic' | 'normal' | 'raidFinder'

export interface Settings {
  language: Language
  /** Groupes de royaumes où l'on a déjà un personnage. */
  ownedRealms: number[]
  /** Coût d'un nouveau royaume, en pièces d'or. */
  newRealmCostGold: number
  /** Liste de cibles du scanner (objets et pièces d'ensembles). */
  targets: Target[]
  /** Version présélectionnée quand un objet est saisi sans sa version. */
  preferredVersion: PreferredVersion
}

const KEY = 'wowscan.settings'
const defaults: Settings = {
  language: navigator.language.startsWith('fr') ? 'fr' : 'en',
  ownedRealms: [],
  newRealmCostGold: 0,
  targets: [],
  preferredVersion: 'cheapest',
}

function load(): Settings {
  try {
    const stored = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    // Version 0.1 : une simple liste d'identifiants.
    if (Array.isArray(stored.itemIds) && !stored.targets) {
      stored.targets = stored.itemIds.map((itemId: number) => ({ kind: 'item', itemId, bonusLists: [] }))
    }
    delete stored.itemIds
    return { ...defaults, ...stored }
  } catch {
    return defaults
  }
}

let current = load()
const listeners = new Set<() => void>()

export const settings = {
  get: () => current,
  update(patch: Partial<Settings>) {
    current = { ...current, ...patch }
    localStorage.setItem(KEY, JSON.stringify(current))
    listeners.forEach((l) => l())
  },
  subscribe(listener: () => void) {
    listeners.add(listener)
    return () => listeners.delete(listener)
  },
}

export function useSettings(): Settings {
  return useSyncExternalStore(settings.subscribe, settings.get)
}
