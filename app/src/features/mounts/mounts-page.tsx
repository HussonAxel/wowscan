import { useQuery } from '@tanstack/react-query'
import { ChevronDown, Gem } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ArcScope } from '@/components/arc-scope'
import { EmptyState } from '@/components/arc/empty-state/empty-state'
import { HoverCard, HoverCardContent, HoverCardTrigger } from '@/components/ui/hover-card'
import { Skeleton } from '@/components/ui/skeleton'
import { AddonCard, useCollectionStatus } from '@/features/collections/collections-page'
import { api } from '@/lib/api'
import { useGameIcon } from '@/lib/game-icon'
import { useErrorMessage } from '@/lib/errors'
import { localized, qualityColor, useApiLocale } from '@/lib/format'
import { openUrl } from '@/lib/open-url'
import { useSettings } from '@/lib/settings'
import type { Mount, MountCategory } from '@/lib/types'
import { cn } from '@/lib/utils'

const CATEGORIES: MountCategory[] = [
  'raid',
  'dungeon',
  'reputation',
  'quest',
  'elite',
  'drop',
  'vendor',
  'achievement',
  'profession',
  'pvp',
  'event',
  'promotion',
  'tradingPost',
  'other',
]

interface CategoryRow {
  category: MountCategory
  mounts: Mount[]
  collected: number
}

interface ExpansionGroup {
  expansion: number
  rows: CategoryRow[]
  collected: number
  total: number
}

const countCollected = (mounts: Mount[]) => mounts.filter((m) => m.collected).length

/** Montures rangées par extension (la plus récente d'abord), puis par catégorie. */
function mountGroups(mounts: Mount[]): ExpansionGroup[] {
  const byExpansion = new Map<number, Mount[]>()
  for (const mount of mounts) byExpansion.set(mount.expansion, [...(byExpansion.get(mount.expansion) ?? []), mount])
  return [...byExpansion]
    .sort(([a], [b]) => b - a)
    .map(([expansion, list]) => ({
      expansion,
      collected: countCollected(list),
      total: list.length,
      rows: CATEGORIES.flatMap((category) => {
        const inCategory = list.filter((m) => m.category === category)
        return inCategory.length > 0 ? [{ category, mounts: inCategory, collected: countCollected(inCategory) }] : []
      }),
    }))
}

function spellUrl(spellId: number, language: string) {
  return `https://www.wowhead.com/${language.startsWith('fr') ? 'fr/' : ''}spell=${spellId}`
}

export function MountsPage() {
  const { t } = useTranslation()
  const status = useCollectionStatus()
  const ready = Boolean(status.data?.gameDir && status.data.scannedAt)
  const addonCard = <AddonCard status={status.data} refreshing={status.isFetching} onRefresh={() => void status.refetch()} />

  return (
    <div className="mx-auto w-full max-w-4xl space-y-6 p-6">
      <header className="space-y-1">
        <h1 className="text-2xl font-semibold tracking-tight">{t('mounts.title')}</h1>
        <p className="max-w-2xl text-sm text-muted-foreground">{t('mounts.subtitle')}</p>
      </header>

      {ready ? (
        <MountOverview scannedAt={status.data?.scannedAt ?? null} addonCard={addonCard} />
      ) : status.isPending ? (
        <Skeleton className="h-24 rounded-2xl" />
      ) : (
        <>
          {addonCard}
          <ArcScope className="rounded-2xl bg-card ring-1 ring-foreground/8">
            <EmptyState icon={<Gem />} title={t('collections.emptyTitle')} description={t('collections.emptyDescription')} />
          </ArcScope>
        </>
      )}
    </div>
  )
}

function MountOverview({ scannedAt, addonCard }: { scannedAt: number | null; addonCard: React.ReactNode }) {
  const { t } = useTranslation()
  const { gameDir } = useSettings()
  const errorMessage = useErrorMessage()
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set())
  const expansions = t('expansions', { returnObjects: true }) as string[]

  const query = useQuery({
    queryKey: ['collection-mounts', gameDir, scannedAt],
    queryFn: () => api.collectionMounts(gameDir || null),
  })
  const groups = useMemo(() => mountGroups(query.data?.mounts ?? []), [query.data])
  const collected = useMemo(() => countCollected(query.data?.mounts ?? []), [query.data])

  const toggle = (expansion: number) =>
    setCollapsed((current) => {
      const next = new Set(current)
      if (!next.delete(expansion)) next.add(expansion)
      return next
    })

  if (query.isPending) return <Skeleton className="h-96 rounded-2xl" />
  if (query.isError) {
    return <p className="rounded-2xl bg-card p-5 text-sm text-destructive ring-1 ring-foreground/8">{errorMessage(query.error)}</p>
  }
  if (!query.data.exported) {
    return (
      <>
        {addonCard}
        <p className="rounded-2xl bg-card p-5 text-sm text-muted-foreground ring-1 ring-foreground/8">{t('mounts.notExported')}</p>
      </>
    )
  }

  return (
    <section className="space-y-6">
      <p className="text-sm font-medium tabular-nums">{t('mounts.summary', { collected, total: query.data.mounts.length })}</p>
      {groups.map((group) => {
        const closed = collapsed.has(group.expansion)
        return (
          <section key={group.expansion} className="space-y-3">
            <h2>
              <button type="button" aria-expanded={!closed} onClick={() => toggle(group.expansion)} className="flex items-center gap-2 text-lg font-semibold">
                <ChevronDown className={cn('size-4 transition-transform', closed && '-rotate-90')} />
                {expansions[group.expansion] ?? group.expansion}
                <span className="text-sm font-normal text-muted-foreground tabular-nums">
                  ({group.collected}/{group.total})
                </span>
              </button>
            </h2>
            {!closed && (
              <div className="flex flex-wrap gap-x-6 gap-y-4">
                {group.rows.map((row) => (
                  <CategoryIcons key={row.category} label={t(`mounts.categories.${row.category}`)} mounts={row.mounts} />
                ))}
              </div>
            )}
          </section>
        )
      })}
      <p className="text-xs text-muted-foreground">{t('mounts.limit')}</p>
    </section>
  )
}

function CategoryIcons({ label, mounts }: { label: string; mounts: Mount[] }) {
  return (
    <div className="min-w-0">
      <h3 className="mb-1 text-sm">{label}</h3>
      <ul className="flex flex-wrap gap-1">
        {mounts.map((mount) => (
          <MountIcon key={mount.id} mount={mount} />
        ))}
      </ul>
    </div>
  )
}

/** Icône de la monture : pleine si elle est obtenue, grisée sinon ; le clic ouvre sa fiche Wowhead. */
function MountIcon({ mount }: { mount: Mount }) {
  const { t, i18n } = useTranslation()
  const locale = useApiLocale()
  const state = t(mount.collected ? 'collections.pieceOwned' : 'collections.pieceMissing')
  const icons = useGameIcon()
  const spellIcon = icons.pick(mount.icon === null ? null : `https://render.worldofwarcraft.com/eu/icons/56/${mount.icon}.jpg`)
  // Sans icône de sort, l'objet qui apprend la monture prend le relais.
  const needsItem = icons.ready && !spellIcon && mount.itemId !== null
  const item = useQuery({
    queryKey: ['item-infos', mount.itemId],
    queryFn: () => api.itemInfos([mount.itemId as number]),
    enabled: needsItem,
    staleTime: Infinity,
  })
  const itemIcon = mount.itemId === null ? undefined : item.data?.[mount.itemId]?.icon
  const src = spellIcon ?? icons.pick(itemIcon)
  return (
    <li>
      <HoverCard openDelay={80} closeDelay={0}>
        <HoverCardTrigger asChild>
          <button
            type="button"
            aria-label={`${localized(mount.name, locale)} (${state})`}
            onClick={() => openUrl(spellUrl(mount.spellId, i18n.language))}
            className="block rounded-md focus-visible:outline-2 focus-visible:outline-primary"
          >
            {src ? (
              <img
                src={src}
                alt=""
                loading="lazy"
                onError={() => icons.fail(src)}
                className={cn('size-9 rounded-md object-cover ring-1 ring-foreground/10', !mount.collected && 'opacity-35 grayscale')}
              />
            ) : (
              <span className="block size-9 rounded-md bg-foreground/[0.06] ring-1 ring-foreground/10" />
            )}
          </button>
        </HoverCardTrigger>
        <HoverCardContent className="w-80 text-sm">
          <MountCard mount={mount} />
        </HoverCardContent>
      </HoverCard>
    </li>
  )
}

/** La fiche du jeu, reconstruite ; nom et source restent affichés si l'API ne répond pas. */
function MountCard({ mount }: { mount: Mount }) {
  const locale = useApiLocale()
  const { data: tooltip } = useQuery({
    queryKey: ['mount-tooltip', mount.id],
    queryFn: () => api.mountTooltip(mount.id, mount.itemId),
  })
  const sources = localized(mount.source, locale).split(' · ').filter(Boolean)
  return (
    <div className="space-y-2">
      <div>
        <div className={cn('text-base font-semibold', qualityColor[tooltip?.quality ?? ''])}>{localized(mount.name, locale)}</div>
        {tooltip?.level && <div className="text-amber-300">{localized(tooltip.level, locale)}</div>}
        {tooltip?.binding && <div>{localized(tooltip.binding, locale)}</div>}
        {tooltip?.kind && <div>{localized(tooltip.kind, locale)}</div>}
      </div>
      {tooltip && tooltip.spells.length > 0 && (
        <div className="space-y-1 text-q-uncommon">
          {tooltip.spells.map((spell) => (
            <p key={localized(spell, locale)}>{localized(spell, locale)}</p>
          ))}
        </div>
      )}
      {tooltip && tooltip.requirements.length > 0 && (
        <div>
          {tooltip.requirements.map((requirement) => (
            <div key={localized(requirement, locale)}>{localized(requirement, locale)}</div>
          ))}
        </div>
      )}
      {tooltip?.description && <p className="text-amber-300">“{localized(tooltip.description, locale)}”</p>}
      <div>
        {sources.map((line) => {
          const [, heading, value] = /^(.+?\s*:)\s*(.*)$/.exec(line) ?? [undefined, undefined, line]
          return (
            <div key={line}>
              {heading && <span className="text-amber-300">{heading} </span>}
              {value}
            </div>
          )
        })}
      </div>
    </div>
  )
}
