import { useTranslation } from 'react-i18next'

import type { Copper } from '@/lib/types'
import { cn } from '@/lib/utils'

const coinColor = { g: 'bg-coin-gold', s: 'bg-coin-silver', c: 'bg-coin-copper' } as const
type Coin = keyof typeof coinColor

/** Pièces affichées : au-delà de 100 po, l'argent et le cuivre ne comptent plus. */
function coins(copper: Copper): [number, Coin][] {
  const gold = Math.floor(copper / 10_000)
  const silver = Math.floor(copper / 100) % 100
  const rest = copper % 100
  if (gold >= 100) return [[gold, 'g']]
  if (gold > 0) return [[gold, 'g'], [silver, 's']]
  if (silver > 0) return [[silver, 's'], [rest, 'c']]
  return [[rest, 'c']]
}

/** Prix façon jeu (12 345 ●), chiffres alignés pour les colonnes. */
export function Money({ copper, className }: { copper: Copper; className?: string }) {
  // Séparateur de milliers de la langue choisie (« 2 500 » en français), pas celle du système.
  const { i18n } = useTranslation()
  return (
    <span className={cn('inline-flex items-center gap-1.5 tabular-nums whitespace-nowrap', className)}>
      {coins(copper).map(([value, coin], i) => (
        <span key={coin} className="inline-flex items-center gap-1">
          {i === 0 ? value.toLocaleString(i18n.language) : String(value).padStart(2, '0')}
          <span aria-hidden className={cn('size-2 rounded-full', coinColor[coin])} />
          <span className="sr-only">{coin}</span>
        </span>
      ))}
    </span>
  )
}
