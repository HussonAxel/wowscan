import { ChevronDown, Layers } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ItemIcon } from '@/components/item-icon'
import { Money } from '@/components/money'
import { Button } from '@/components/ui/button'
import { localized, qualityColor, realmName, useApiLocale } from '@/lib/format'
import type { ConnectedRealm, ItemInfo, ItemResult } from '@/lib/types'
import { cn } from '@/lib/utils'

import { RealmPriceTable, type RealmPriceRow } from './realm-price-table'

const COLLAPSED_ROWS = 8

export function ItemResultCard({
  result,
  info,
  realms,
  owned,
}: {
  result: ItemResult
  info?: ItemInfo
  realms: Map<number, ConnectedRealm>
  owned: Set<number>
}) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const [expanded, setExpanded] = useState(false)

  const rows = useMemo<RealmPriceRow[]>(
    () =>
      result.realms.map((r, i) => ({
        connectedRealmId: r.connectedRealmId,
        realm: realmName(realms.get(r.connectedRealmId), locale),
        price: r.stats.min,
        auctions: r.stats.auctions,
        owned: owned.has(r.connectedRealmId),
        best: i === 0,
      })),
    [result.realms, realms, owned, locale],
  )
  const best = result.commodity?.min ?? result.realms[0]?.stats.min

  return (
    <section className="overflow-hidden rounded-xl bg-card ring-1 ring-foreground/8">
      <header className="flex items-center gap-3 p-4">
        <ItemIcon item={info} />
        <div className="min-w-0 flex-1">
          <h3 className={cn('truncate font-medium', qualityColor[info?.quality ?? ''])}>
            {info ? localized(info.name, locale) : t('results.unknownItem', { id: result.itemId })}
          </h3>
          <p className="truncate text-xs text-muted-foreground">
            {info && [localized(info.itemSubclass, locale), localized(info.inventoryType, locale)].filter(Boolean).join(' · ')}
            {info && ' · '}#{result.itemId}
          </p>
        </div>
        {best !== undefined && (
          <div className="text-right">
            <div className="text-xs text-muted-foreground">{t('results.best')}</div>
            <Money copper={best} className="text-base font-semibold" />
          </div>
        )}
      </header>

      {result.commodity ? (
        <p className="flex items-center gap-2 border-t border-foreground/8 px-4 py-3 text-sm text-muted-foreground">
          <Layers className="size-4" />
          {t('results.commodity')} · {result.commodity.quantity.toLocaleString()} {t('results.available').toLowerCase()}
        </p>
      ) : rows.length === 0 ? (
        <p className="border-t border-foreground/8 px-4 py-3 text-sm text-muted-foreground">{t('results.noAuction')}</p>
      ) : (
        <div className="border-t border-foreground/8">
          <RealmPriceTable rows={rows} limit={expanded ? undefined : COLLAPSED_ROWS} />
          {rows.length > COLLAPSED_ROWS && (
            <Button variant="ghost" size="sm" className="m-1 text-muted-foreground" onClick={() => setExpanded((e) => !e)}>
              <ChevronDown className={cn('transition-transform duration-300', expanded && 'rotate-180')} />
              {expanded ? t('results.showLess') : t('results.showAll', { count: rows.length })}
            </Button>
          )}
        </div>
      )}
    </section>
  )
}
