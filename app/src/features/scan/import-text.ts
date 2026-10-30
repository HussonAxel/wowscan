import type { TFunction } from 'i18next'
import { toast } from 'sonner'

import { api } from '@/lib/api'
import { settings } from '@/lib/settings'
import { type Target, targetKey } from '@/lib/targets'

/**
 * Ajoute au scanner ce que contient un texte collé : liens en jeu (avec leur version),
 * liens Wowhead (?bonus=), identifiants, ou noms entre crochets d'une copie du chat.
 * Renvoie le nombre de cibles ajoutées.
 */
export async function importText(text: string, t: TFunction): Promise<number> {
  const parsed = await api.parseItems(text)
  const found: Target[] = parsed.items.map((i) => ({ kind: 'item', ...i }))

  if (parsed.names.length) {
    const byName = await api.findItemsByName(parsed.names)
    const missing = parsed.names.filter((n) => !byName[n]?.length)
    for (const name of parsed.names) {
      for (const itemId of byName[name] ?? []) found.push({ kind: 'item', itemId, bonusLists: [] })
    }
    // Les noms de joueurs ou de canaux d'une ligne de chat ne sont pas des objets : on ne
    // les signale que si le texte ne contenait rien d'autre.
    if (missing.length && !found.length) toast.warning(t('scan.namesNotFound', { names: missing.join(', ') }))
  }
  if (parsed.battlePets.length) toast.info(t('scan.petsUnsupported', { names: parsed.battlePets.join(', ') }))
  if (parsed.appearances.length) toast.info(t('scan.appearanceLinks', { names: parsed.appearances.join(', ') }))
  if (parsed.unrecognized.length) toast.warning(t('scan.unrecognized', { tokens: parsed.unrecognized.join(', ') }))

  const current = settings.get().targets
  const known = new Set(current.map(targetKey))
  const added = found.filter((target) => {
    const key = targetKey(target)
    return !known.has(key) && known.add(key)
  })
  if (added.length) settings.update({ targets: [...current, ...added] })
  else if (!found.length && !parsed.battlePets.length && !parsed.appearances.length && !parsed.names.length) {
    toast.warning(t('scan.nothingFound'))
  }
  return added.length
}
