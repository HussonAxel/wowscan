import { ChevronDown, Crosshair, Layers } from 'lucide-react'
import { motion } from 'motion/react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ItemIcon } from '@/components/item-icon'
import { Money } from '@/components/money'
import { Button } from '@/components/ui/button'
import { localized, qualityColor, realmName, useApiLocale } from '@/lib/format'
import { optionLabel } from '@/lib/labels'
import type { Target, TargetOption } from '@/lib/targets'
import type { ConnectedRealm, ItemInfo, LocalizedString } from '@/lib/types'
import { cn } from '@/lib/utils'

import { RealmPriceTable, type RealmPriceRow } from './realm-price-table'

const COLLAPSED_ROWS = 6

export interface TargetView {
  key: string
  target: Target
  options: TargetOption[]
  chosen?: TargetOption
  /** Apparence de la version demandée (bonus du lien), si elle est connue. */
  requestedAppearance: number | null
}

export function TargetCard({
  view,
  infos,
  labelTexts,
  realms,
  owned,
  onSelect,
}: {
  view: TargetView
  infos: Record<number, ItemInfo>
  labelTexts: Record<number, LocalizedString>
  realms: Map<number, ConnectedRealm>
  owned: Set<number>
  onSelect: (optionKey: string) => void
}) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const [expanded, setExpanded] = useState(false)
  const { target, options, chosen, requestedAppearance } = view
  const info = infos[target.itemId]
  const requestedMissing = requestedAppearance != null && !options.some((o) => o.appearanceId === requestedAppearance)

  const rows = useMemo<RealmPriceRow[]>(
    () =>
      (chosen?.realms ?? []).map((r, i) => ({
        connectedRealmId: r.connectedRealmId,
        realm: realmName(realms.get(r.connectedRealmId), locale),
        price: r.stats.min,
        auctions: r.stats.auctions,
        owned: owned.has(r.connectedRealmId),
        best: i === 0,
        note: r.itemId !== target.itemId && infos[r.itemId] ? localized(infos[r.itemId].name, locale) : undefined,
      })),
    [chosen, realms, owned, locale, infos, target.itemId],
  )

  const subtitle = [
    info && localized(info.itemSubclass, locale),
    info && localized(info.inventoryType, locale),
    target.kind === 'appearance'
      ? target.itemIds.length > 1 && t('sets.sources', { count: target.itemIds.length })
      : `#${target.itemId}`,
  ]
    .filter(Boolean)
    .join(' · ')

  return (
    <section className="overflow-hidden rounded-2xl bg-card ring-1 ring-foreground/8">
      <header className="flex items-center gap-3 p-4">
        <ItemIcon item={info} className="size-10" />
        <div className="min-w-0 flex-1">
          <h3 className={cn('truncate font-medium', qualityColor[info?.quality ?? ''])}>
            {info ? localized(info.name, locale) : t('results.unknownItem', { id: target.itemId })}
          </h3>
          <p className="truncate text-xs text-muted-foreground">{subtitle}</p>
        </div>
        {chosen?.best != null && (
          <div className="text-right">
            <div className="text-xs text-muted-foreground">{t('results.best')}</div>
            <Money copper={chosen.best} className="text-base font-semibold" />
          </div>
        )}
      </header>

      {requestedMissing && <p className="px-4 pb-2 text-xs text-amber-300/90">{t('versions.notForSale')}</p>}

      {(options.length > 1 || requestedAppearance != null) && (
        <div role="radiogroup" aria-label={t('versions.requested')} className="flex flex-wrap gap-1.5 px-4 pb-3">
          {options.map((option) => {
            const selected = option.key === chosen?.key
            const requested = option.appearanceId != null && option.appearanceId === requestedAppearance
            return (
              <button
                key={option.key}
                type="button"
                role="radio"
                aria-checked={selected}
                onClick={() => onSelect(option.key)}
                className={cn(
                  'relative inline-flex h-8 items-center gap-2 rounded-full px-3 text-sm transition-colors duration-150',
                  selected ? 'text-foreground' : 'text-muted-foreground ring-1 ring-foreground/10 hover:bg-foreground/[0.04] hover:text-foreground',
                )}
              >
                {selected && (
                  <motion.span
                    layoutId={`variant-${view.key}`}
                    className="absolute inset-0 rounded-full bg-primary/12 ring-1 ring-primary/45"
                    transition={{ type: 'spring', bounce: 0.2, duration: 0.35 }}
                  />
                )}
                <span className="relative flex items-center gap-1.5">
                  {requested && <Crosshair className="size-3.5 text-primary" aria-label={t('versions.requested')} />}
                  {optionLabel(option, target.kind, labelTexts, locale, t)}
                </span>
                {option.best != null && <Money copper={option.best} className="relative text-xs opacity-80" />}
              </button>
            )
          })}
        </div>
      )}

      {chosen?.commodity ? (
        <p className="flex items-center gap-2 border-t border-foreground/8 px-4 py-3 text-sm text-muted-foreground">
          <Layers className="size-4" />
          {t('results.commodity')} · {chosen.commodity.quantity.toLocaleString()} {t('results.available').toLowerCase()}
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
