import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { settings } from '@/lib/settings'
import { type Target, targetKey } from '@/lib/targets'
import type { SetDetails, SetSummary } from '@/lib/types'

const ALL_CLASSES_MASK = (1 << 13) - 1

export function useDebounced<T>(value: T, ms: number) {
  const [debounced, setDebounced] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), ms)
    return () => clearTimeout(timer)
  }, [value, ms])
  return debounced
}

/** « Mage, Prêtre, Démoniste », ou « Toutes les classes ». */
export function useClassText() {
  const { t } = useTranslation()
  const names = t('classes', { returnObjects: true }) as string[]
  return (mask: number) => {
    if (!mask || (mask & ALL_CLASSES_MASK) === ALL_CLASSES_MASK) return t('sets.allClasses')
    return names.filter((name, classId) => classId > 0 && name && mask & (1 << (classId - 1))).join(', ')
  }
}

export function useSetMeta() {
  const { t } = useTranslation()
  const expansions = t('expansions', { returnObjects: true }) as string[]
  const classText = useClassText()
  return (set: SetSummary) => [expansions[set.expansion], classText(set.classMask)].filter(Boolean).join(' · ')
}

/** Une cible « apparence » par pièce : n'importe quel objet qui la donne fait l'affaire. */
export function setTargets(set: SetDetails): Target[] {
  return set.pieces.map((piece) => ({
    kind: 'appearance',
    appearanceId: piece.appearanceId,
    itemIds: [...new Set(piece.allSources.map((s) => s.itemId))],
    itemId: piece.setSources[0].itemId,
    setId: set.id,
  }))
}

export function addTargets(targets: Target[]) {
  const current = settings.get().targets
  const known = new Set(current.map(targetKey))
  const added = targets.filter((t) => !known.has(targetKey(t)))
  settings.update({ targets: [...current, ...added] })
  return added.length
}
