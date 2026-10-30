import { useQuery } from '@tanstack/react-query'

import { api } from '@/lib/api'
import type { Target } from '@/lib/targets'
import { targetKey } from '@/lib/targets'
import type { Variant } from '@/lib/types'

/** Libellés de version (« Mythique »…), communs à toute l'app. */
export function useVariantLabels() {
  return useQuery({ queryKey: ['variant-labels'], queryFn: api.variantLabels }).data ?? {}
}

/** Version demandée de chaque cible saisie avec ses bonus, par clé de cible. */
export function useRequestedVariants(targets: Target[]): Record<string, Variant> {
  const withBonus = targets.filter((t) => t.kind === 'item' && t.bonusLists.length > 0)
  const refs = withBonus.map((t) => ({ itemId: t.itemId, bonusLists: t.kind === 'item' ? t.bonusLists : [] }))
  const { data } = useQuery({
    queryKey: ['requested-variants', refs],
    queryFn: () => api.resolveVariants(refs),
    enabled: refs.length > 0,
    placeholderData: (previous) => previous,
  })
  if (!data || data.length !== withBonus.length) return {}
  return Object.fromEntries(withBonus.map((t, i) => [targetKey(t), data[i]]))
}
