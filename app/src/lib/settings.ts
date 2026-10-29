import { useSyncExternalStore } from 'react'

import type { Language } from '@/i18n'

/** Préférences locales. Les identifiants Blizzard n'y sont pas : ils vont dans le coffre du système. */
export interface Settings {
  language: Language
  /** Groupes de royaumes où l'on a déjà un personnage. */
  ownedRealms: number[]
  /** Coût d'un nouveau royaume, en pièces d'or. */
  newRealmCostGold: number
  /** Dernière liste d'objets saisie. */
  itemIds: number[]
}

const KEY = 'wowscan.settings'
const defaults: Settings = {
  language: navigator.language.startsWith('fr') ? 'fr' : 'en',
  ownedRealms: [],
  newRealmCostGold: 0,
  itemIds: [],
}

function load(): Settings {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(KEY) ?? '{}') }
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
