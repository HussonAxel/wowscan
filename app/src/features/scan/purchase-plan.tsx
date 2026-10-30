import { useQuery } from '@tanstack/react-query'
import { MapPin, UserPlus } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { ItemIcon } from '@/components/item-icon'
import { AnimatedNumber } from '@/components/motion/animated-number'
import { Money } from '@/components/money'
import { Badge } from '@/components/ui/badge'
import { api } from '@/lib/api'
import { qualityColor, realmName, useApiLocale } from '@/lib/format'
import { useSettings } from '@/lib/settings'
import type { TargetOption } from '@/lib/targets'
import type { ConnectedRealm, ItemInfo } from '@/lib/types'
import { cn } from '@/lib/utils'

/** Une ligne de la liste d'achat : une cible et la version choisie. */
export interface PlanLine {
  key: string
  title: string
  version: string
  info?: ItemInfo
  option: TargetOption
}

/** Où acheter chaque ligne pour payer le moins, en comptant le coût d'un nouveau royaume. */
export function PurchasePlan({ lines, realms }: { lines: PlanLine[]; realms: Map<number, ConnectedRealm> }) {
  const { t, i18n } = useTranslation()
  const locale = useApiLocale()
  const { ownedRealms, newRealmCostGold } = useSettings()

  const perRealm = lines.filter((l) => !l.option.commodity)
  const commodities = lines.filter((l) => l.option.commodity)
  const input = {
    // La clé d'une ligne d'achat est son rang : un objet peut être demandé en deux versions.
    wants: perRealm.map((l, i) => ({
      itemId: i,
      candidates: l.option.realms.map((r) => ({ connectedRealmId: r.connectedRealmId, price: r.stats.min })),
    })),
    ownedRealms,
    newRealmCost: newRealmCostGold * 10_000,
  }
  const { data: plan } = useQuery({
    queryKey: ['plan', perRealm.map((l) => `${l.key}=${l.option.key}`), ownedRealms, newRealmCostGold],
    queryFn: () => api.plan(input),
    enabled: perRealm.length > 0,
    placeholderData: (previous) => previous,
  })

  const commoditiesCost = commodities.reduce((sum, l) => sum + (l.option.commodity?.min ?? 0), 0)
  const total = (perRealm.length ? (plan?.total ?? 0) : 0) + commoditiesCost

  return (
    <aside className="space-y-4 rounded-2xl bg-card p-4 ring-1 ring-foreground/8">
      <div>
        <h2 className="font-semibold tracking-tight">{t('plan.title')}</h2>
        <p className="text-sm text-muted-foreground">{t('plan.description')}</p>
      </div>

      <div className="rounded-xl bg-primary/[0.07] p-3 ring-1 ring-primary/20">
        <div className="text-xs text-muted-foreground">{t('plan.total')}</div>
        <div className="flex items-center gap-1.5 text-2xl font-semibold tabular-nums text-primary">
          <AnimatedNumber value={Math.floor(total / 10_000)} format={(n) => Math.round(n).toLocaleString(i18n.language)} />
          <span aria-hidden className="size-2.5 rounded-full bg-coin-gold" />
        </div>
        {plan && plan.newRealms.length > 0 && (
          <div className="mt-1 flex justify-between text-xs text-muted-foreground">
            <span>
              {t('plan.newRealms')} · {plan.newRealms.length}
            </span>
            <Money copper={plan.newRealmsCost} />
          </div>
        )}
      </div>

      {plan?.realms.map((realmId) => {
        const isNew = plan.newRealms.includes(realmId)
        return (
          <div key={realmId} className="space-y-1.5">
            <div className="flex items-center gap-2 text-sm font-medium">
              {isNew ? <UserPlus className="size-4 shrink-0 text-muted-foreground" /> : <MapPin className="size-4 shrink-0 text-primary" />}
              <span className="truncate">{realmName(realms.get(realmId), locale)}</span>
              <Badge variant="outline" className={cn('ml-auto shrink-0', !isNew && 'border-primary/30 text-primary')}>
                {isNew ? t('plan.newCharacter') : t('plan.ownedRealm')}
              </Badge>
            </div>
            <ul className="space-y-1 pl-6">
              {plan.purchases
                .filter((p) => p.connectedRealmId === realmId)
                .map((p) => (
                  <Line key={p.itemId} line={perRealm[p.itemId]} copper={p.price} />
                ))}
            </ul>
          </div>
        )
      })}

      {commodities.length > 0 && (
        <div className="space-y-1.5">
          <div className="text-sm font-medium">{t('plan.commodities')}</div>
          <ul className="space-y-1 pl-6">
            {commodities.map((l) => (
              <Line key={l.key} line={l} copper={l.option.commodity!.min} />
            ))}
          </ul>
        </div>
      )}

      {plan && plan.unavailable.length > 0 && (
        <p className="text-sm text-muted-foreground">
          {t('plan.unavailable', { names: plan.unavailable.map((i) => perRealm[i]?.title).join(', ') })}
        </p>
      )}
      <p className="border-t border-foreground/8 pt-3 text-xs text-muted-foreground">{t('plan.tip')}</p>
    </aside>
  )
}

function Line({ line, copper }: { line?: PlanLine; copper: number }) {
  if (!line) return null
  return (
    <li className="flex items-center gap-2 text-sm">
      <ItemIcon item={line.info} className="size-5 rounded" />
      <span className="min-w-0 flex-1 truncate">
        <span className={qualityColor[line.info?.quality ?? '']}>{line.title}</span>
        {line.version && <span className="text-muted-foreground"> · {line.version}</span>}
      </span>
      <Money copper={copper} className="text-muted-foreground" />
    </li>
  )
}
