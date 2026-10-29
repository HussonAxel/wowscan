import { useQuery } from '@tanstack/react-query'
import { MapPin, UserPlus } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { AnimatedNumber } from '@/components/motion/animated-number'
import { ItemIcon } from '@/components/item-icon'
import { Money } from '@/components/money'
import { Badge } from '@/components/ui/badge'
import { api } from '@/lib/api'
import { localized, qualityColor, realmName, useApiLocale } from '@/lib/format'
import { useSettings } from '@/lib/settings'
import type { ConnectedRealm, ItemInfo, ScanResult } from '@/lib/types'
import { cn } from '@/lib/utils'

/** Où acheter chaque objet pour payer le moins, en comptant le coût d'un nouveau royaume. */
export function PurchasePlan({
  result,
  infos,
  realms,
}: {
  result: ScanResult
  infos: Record<number, ItemInfo>
  realms: Map<number, ConnectedRealm>
}) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const { ownedRealms, newRealmCostGold } = useSettings()

  const perRealm = result.items.filter((i) => !i.commodity)
  const commodities = result.items.filter((i) => i.commodity)
  const input = {
    wants: perRealm.map((i) => ({
      itemId: i.itemId,
      candidates: i.realms.map((r) => ({ connectedRealmId: r.connectedRealmId, price: r.stats.min })),
    })),
    ownedRealms,
    newRealmCost: newRealmCostGold * 10_000,
  }
  const { data: plan } = useQuery({
    queryKey: ['plan', result.scannedAt, result.items.map((i) => i.itemId), ownedRealms, newRealmCostGold],
    queryFn: () => api.plan(input),
    enabled: perRealm.length > 0,
  })

  const commoditiesCost = commodities.reduce((sum, i) => sum + (i.commodity?.min ?? 0), 0)
  const total = (plan?.total ?? 0) + commoditiesCost
  const name = (id: number) => (infos[id] ? localized(infos[id].name, locale) : `#${id}`)

  return (
    <aside className="space-y-4 rounded-xl bg-card p-4 ring-1 ring-foreground/8">
      <div>
        <h2 className="font-semibold tracking-tight">{t('plan.title')}</h2>
        <p className="text-sm text-muted-foreground">{t('plan.description')}</p>
      </div>

      <div className="rounded-lg bg-primary/[0.07] p-3 ring-1 ring-primary/20">
        <div className="text-xs text-muted-foreground">{t('plan.total')}</div>
        <div className="flex items-center gap-1.5 text-2xl font-semibold tabular-nums text-primary">
          <AnimatedNumber value={Math.floor(total / 10_000)} format={(n) => Math.round(n).toLocaleString()} />
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
              {isNew ? <UserPlus className="size-4 text-muted-foreground" /> : <MapPin className="size-4 text-primary" />}
              <span className="truncate">{realmName(realms.get(realmId), locale)}</span>
              <Badge variant="outline" className={cn('ml-auto shrink-0', !isNew && 'border-primary/30 text-primary')}>
                {isNew ? t('plan.newCharacter') : t('plan.ownedRealm')}
              </Badge>
            </div>
            <ul className="space-y-1 pl-6">
              {plan.purchases
                .filter((p) => p.connectedRealmId === realmId)
                .map((p) => (
                  <PlanLine key={p.itemId} info={infos[p.itemId]} name={name(p.itemId)} copper={p.price} />
                ))}
            </ul>
          </div>
        )
      })}

      {commodities.length > 0 && (
        <div className="space-y-1.5">
          <div className="text-sm font-medium">{t('plan.commodities')}</div>
          <ul className="space-y-1 pl-6">
            {commodities.map((i) => (
              <PlanLine key={i.itemId} info={infos[i.itemId]} name={name(i.itemId)} copper={i.commodity!.min} />
            ))}
          </ul>
        </div>
      )}

      {plan && plan.unavailable.length > 0 && (
        <p className="text-sm text-muted-foreground">{t('plan.unavailable', { names: plan.unavailable.map(name).join(', ') })}</p>
      )}
      <p className="border-t border-foreground/8 pt-3 text-xs text-muted-foreground">{t('plan.tip')}</p>
    </aside>
  )
}

function PlanLine({ info, name, copper }: { info?: ItemInfo; name: string; copper: number }) {
  return (
    <li className="flex items-center gap-2 text-sm">
      <ItemIcon item={info} className="size-5 rounded" />
      <span className={cn('min-w-0 flex-1 truncate', qualityColor[info?.quality ?? ''])}>{name}</span>
      <Money copper={copper} className="text-muted-foreground" />
    </li>
  )
}
