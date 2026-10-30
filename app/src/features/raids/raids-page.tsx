import { useQueries, useQuery } from '@tanstack/react-query'
import { Check, ChevronDown, Gem } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ArcScope } from '@/components/arc-scope'
import { EmptyState } from '@/components/arc/empty-state/empty-state'
import { ItemIcon } from '@/components/item-icon'
import { HoverCard, HoverCardContent, HoverCardTrigger } from '@/components/ui/hover-card'
import { Skeleton } from '@/components/ui/skeleton'
import { AddonCard, slotRank, useCollectionStatus, wowheadUrl } from '@/features/collections/collections-page'
import { api } from '@/lib/api'
import { useErrorMessage } from '@/lib/errors'
import { localized, useApiLocale } from '@/lib/format'
import { openUrl } from '@/lib/open-url'
import { useSettings } from '@/lib/settings'
import type { ItemInfo, SetCollection, SetDifficulty, SetPiece } from '@/lib/types'
import { cn } from '@/lib/utils'

const DIFFICULTIES: SetDifficulty[] = ['raidFinder', 'normal', 'heroic', 'mythic']

/** Icônes de classe, à l'indice de l'identifiant de classe du jeu. */
const CLASS_ICONS = [
  '', 'warrior', 'paladin', 'hunter', 'rogue', 'priest', 'deathknight', 'shaman', 'mage', 'warlock', 'monk', 'druid',
  'demonhunter', 'evoker',
]
const CLASS_IDS = CLASS_ICONS.flatMap((icon, id) => (icon ? [id] : []))

interface RaidRow {
  instanceId: number
  name: SetCollection['name']
  /** Ensembles de la difficulté choisie. */
  sets: SetCollection[]
}

interface ExpansionGroup {
  expansion: number
  raids: RaidRow[]
}

const allowsClass = (set: SetCollection, classId: number) => set.classMask === 0 || (set.classMask & (1 << (classId - 1))) !== 0

/** Raids de la difficulté, rangés par extension (la plus récente d'abord). */
function raidGroups(sets: SetCollection[], difficulty: SetDifficulty): ExpansionGroup[] {
  // L'extension d'un raid est la plus fréquente parmi tous ses ensembles : quelques-uns sont mal rangés dans les tables.
  const expansions = new Map<number, Map<number, number>>()
  for (const set of sets) {
    if (!set.raid) continue
    const counts = expansions.get(set.raid.instanceId) ?? new Map<number, number>()
    counts.set(set.expansion, (counts.get(set.expansion) ?? 0) + 1)
    expansions.set(set.raid.instanceId, counts)
  }
  const raids = new Map<number, RaidRow>()
  for (const set of sets) {
    if (set.raid?.difficulty !== difficulty || set.total === 0) continue
    const raid = raids.get(set.raid.instanceId) ?? { instanceId: set.raid.instanceId, name: set.raid.name, sets: [] }
    raid.sets.push(set)
    raids.set(set.raid.instanceId, raid)
  }
  const groups = new Map<number, RaidRow[]>()
  for (const raid of raids.values()) {
    const counts = [...(expansions.get(raid.instanceId) ?? [])]
    const expansion = counts.reduce((best, entry) => (entry[1] > best[1] ? entry : best), counts[0])[0]
    groups.set(expansion, [...(groups.get(expansion) ?? []), raid])
  }
  return [...groups]
    .sort(([a], [b]) => b - a)
    .map(([expansion, list]) => ({ expansion, raids: list.sort((a, b) => b.instanceId - a.instanceId) }))
}

export function RaidsPage() {
  const { t } = useTranslation()
  const status = useCollectionStatus()
  const ready = Boolean(status.data?.gameDir && status.data.scannedAt)

  return (
    <div className="mx-auto w-full max-w-7xl space-y-6 p-6">
      <header className="space-y-1">
        <h1 className="text-2xl font-semibold tracking-tight">{t('raids.title')}</h1>
        <p className="max-w-2xl text-sm text-muted-foreground">{t('raids.subtitle')}</p>
      </header>

      {ready ? (
        <RaidOverview scannedAt={status.data?.scannedAt ?? null} />
      ) : status.isPending ? (
        <Skeleton className="h-24 rounded-2xl" />
      ) : (
        <>
          <AddonCard status={status.data} refreshing={status.isFetching} onRefresh={() => void status.refetch()} />
          <ArcScope className="rounded-2xl bg-card ring-1 ring-foreground/8">
            <EmptyState icon={<Gem />} title={t('collections.emptyTitle')} description={t('collections.emptyDescription')} />
          </ArcScope>
        </>
      )}
    </div>
  )
}

function RaidOverview({ scannedAt }: { scannedAt: number | null }) {
  const { t } = useTranslation()
  const { gameDir } = useSettings()
  const locale = useApiLocale()
  const errorMessage = useErrorMessage()
  const [difficulty, setDifficulty] = useState<SetDifficulty>('normal')
  const [collapsed, setCollapsed] = useState<ReadonlySet<number>>(new Set())
  const classes = t('classes', { returnObjects: true }) as string[]
  const expansions = t('expansions', { returnObjects: true }) as string[]

  const query = useQuery({
    queryKey: ['collection-sets', gameDir, scannedAt],
    queryFn: () => api.collectionSets(gameDir || null),
  })
  const groups = useMemo(() => raidGroups(query.data ?? [], difficulty), [query.data, difficulty])

  const toggle = (expansion: number) =>
    setCollapsed((current) => {
      const next = new Set(current)
      if (!next.delete(expansion)) next.add(expansion)
      return next
    })

  return (
    <section className="space-y-3">
      <div role="tablist" aria-label={t('raids.difficulty')} className="inline-flex rounded-lg bg-foreground/[0.05] p-1">
        {DIFFICULTIES.map((d) => (
          <button
            key={d}
            type="button"
            role="tab"
            aria-selected={d === difficulty}
            onClick={() => setDifficulty(d)}
            className={cn(
              'rounded-md px-3 py-1.5 text-sm font-medium transition-colors',
              d === difficulty ? 'bg-card shadow-sm ring-1 ring-foreground/8' : 'text-muted-foreground hover:text-foreground',
            )}
          >
            {t(`raids.difficulties.${d}`)}
          </button>
        ))}
      </div>

      {query.isPending ? (
        <Skeleton className="h-96 rounded-2xl" />
      ) : query.isError ? (
        <p className="rounded-2xl bg-card p-5 text-sm text-destructive ring-1 ring-foreground/8">{errorMessage(query.error)}</p>
      ) : groups.length === 0 ? (
        <p className="rounded-2xl bg-card p-5 text-sm text-muted-foreground ring-1 ring-foreground/8">{t('raids.empty')}</p>
      ) : (
        <div className="max-h-[calc(100vh-13rem)] overflow-auto rounded-2xl bg-card ring-1 ring-foreground/8">
          <table className="w-full border-separate border-spacing-0 text-sm">
            <thead>
              <tr>
                <th scope="col" className="sticky top-0 left-0 z-30 min-w-56 bg-card px-4 py-2 text-left font-medium text-muted-foreground">
                  {t('raids.raid')}
                </th>
                {CLASS_IDS.map((id) => (
                  <th key={id} scope="col" title={classes[id]} className="sticky top-0 z-20 bg-card px-1 py-2">
                    <img
                      src={`https://render.worldofwarcraft.com/eu/icons/56/classicon_${CLASS_ICONS[id]}.jpg`}
                      alt={classes[id]}
                      loading="lazy"
                      className="mx-auto size-6 rounded-md"
                    />
                  </th>
                ))}
              </tr>
            </thead>
            {groups.map((group) => {
              const closed = collapsed.has(group.expansion)
              return (
                <tbody key={group.expansion}>
                  <tr>
                    <th colSpan={CLASS_IDS.length + 1} scope="colgroup" className="border-t border-foreground/8 bg-foreground/[0.03] p-0 text-left">
                      <button
                        type="button"
                        aria-expanded={!closed}
                        onClick={() => toggle(group.expansion)}
                        className="sticky left-0 flex items-center gap-2 px-4 py-2 font-semibold"
                      >
                        <ChevronDown className={cn('size-4 transition-transform', closed && '-rotate-90')} />
                        {expansions[group.expansion] ?? group.expansion}
                      </button>
                    </th>
                  </tr>
                  {!closed &&
                    group.raids.map((raid) => (
                      <tr key={raid.instanceId} className="group/row">
                        <th
                          scope="row"
                          className="sticky left-0 z-10 max-w-64 truncate border-t border-foreground/5 bg-card px-4 py-2 text-left font-normal group-hover/row:bg-muted"
                          title={localized(raid.name, locale)}
                        >
                          {localized(raid.name, locale)}
                        </th>
                        {CLASS_IDS.map((id) => (
                          <td key={id} className="border-t border-foreground/5 px-1 py-2 text-center group-hover/row:bg-foreground/[0.03]">
                            <ClassCell raid={raid} classId={id} className={classes[id] ?? ''} />
                          </td>
                        ))}
                      </tr>
                    ))}
                </tbody>
              )
            })}
          </table>
        </div>
      )}
      <p className="text-xs text-muted-foreground">{t('raids.limit')}</p>
    </section>
  )
}

function ClassCell({ raid, classId, className }: { raid: RaidRow; classId: number; className: string }) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const sets = raid.sets.filter((s) => allowsClass(s, classId))
  if (sets.length === 0) return null
  const collected = sets.reduce((sum, s) => sum + s.collected, 0)
  const total = sets.reduce((sum, s) => sum + s.total, 0)
  const complete = collected === total
  const label = t('raids.cell', { raid: localized(raid.name, locale), className, collected, total })

  return (
    <HoverCard openDelay={120} closeDelay={80}>
      <HoverCardTrigger asChild>
        <button type="button" aria-label={label} className="mx-auto flex h-5 w-full min-w-8 items-center justify-center rounded-sm">
          {complete ? (
            <Check className="size-4 text-primary" />
          ) : (
            <span className="block h-1.5 w-full max-w-10 overflow-hidden rounded-full bg-foreground/[0.1]">
              <span className="block h-full rounded-full bg-primary/70" style={{ width: `${(collected / total) * 100}%` }} />
            </span>
          )}
        </button>
      </HoverCardTrigger>
      <HoverCardContent className="max-h-[70vh] w-auto max-w-sm min-w-64 overflow-y-auto">
        <CellDetails raid={raid} sets={sets} className={className} />
      </HoverCardContent>
    </HoverCard>
  )
}

/** Chargé seulement à l'ouverture : les pièces et les icônes ne sont pas utiles avant. */
function CellDetails({ raid, sets, className }: { raid: RaidRow; sets: SetCollection[]; className: string }) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const details = useQueries({
    queries: sets.map((set) => ({ queryKey: ['set', set.id], queryFn: () => api.setDetails(set.id) })),
  })
  const itemIds = [
    ...new Set(details.flatMap((d) => d.data?.pieces.map((p) => p.setSources[0]?.itemId).filter((id) => id !== undefined) ?? [])),
  ].sort((a, b) => a - b)
  const infos = useQuery({
    queryKey: ['raid-cell-items', itemIds],
    queryFn: () => api.itemInfos(itemIds),
    enabled: itemIds.length > 0,
    retry: false,
  })

  return (
    <div className="space-y-3">
      <div className="text-sm font-semibold">
        {localized(raid.name, locale)} · {className}
      </div>
      {sets.map((set, index) => {
        const pieces = details[index]?.data?.pieces
        const missing = new Set(set.missing)
        return (
          <div key={set.id} className="space-y-1.5">
            <div className="flex items-baseline justify-between gap-3 text-xs text-muted-foreground">
              <span className="truncate">{localized(set.name, locale)}</span>
              <span className="shrink-0 tabular-nums">
                {set.collected}/{set.total}
              </span>
            </div>
            {pieces ? (
              <ul className="flex flex-wrap gap-1.5">
                {[...pieces]
                  .sort((a, b) => slotRank(a.slot) - slotRank(b.slot))
                  .map((piece) => (
                    <PieceIcon
                      key={piece.appearanceId}
                      piece={piece}
                      owned={!missing.has(piece.appearanceId)}
                      raidId={raid.instanceId}
                      item={infos.data?.[piece.setSources[0]?.itemId ?? 0]}
                    />
                  ))}
              </ul>
            ) : details[index]?.isError ? (
              <p className="text-xs text-muted-foreground">{t('raids.detailsError')}</p>
            ) : (
              <Skeleton className="h-9 rounded-md" />
            )}
          </div>
        )
      })}
    </div>
  )
}

/** Icône de la pièce : pleine si elle est obtenue, grisée sinon ; le clic ouvre sa fiche Wowhead. */
function PieceIcon({ piece, owned, raidId, item }: { piece: SetPiece; owned: boolean; raidId: number; item?: ItemInfo }) {
  const { t, i18n } = useTranslation()
  const locale = useApiLocale()
  const itemId = piece.setSources[0]?.itemId
  const drops = [...new Map(piece.allSources.flatMap((s) => s.drops).map((d) => [d.encounterId, d])).values()]
  // Le boss du raid affiché d'abord : la même apparence tombe parfois ailleurs.
  const here = drops.filter((d) => d.instanceId === raidId)
  const kinds = [...new Set(piece.allSources.map((s) => s.kind))].filter((k) => k > 0 && k !== 9).sort((a, b) => a - b)
  const source =
    (here.length > 0 ? here : drops)
      .slice(0, 2)
      .map((d) => `${localized(d.encounter, locale)} — ${localized(d.instance, locale)}`)
      .join(' · ') || kinds.map((k) => t(`collections.kinds.${k}`, { defaultValue: '' })).filter(Boolean).join(' · ')
  const slot = piece.slot ? t(`slots.${piece.slot}`, { defaultValue: '' }) : ''
  const name = item ? localized(item.name, locale) : slot || `#${itemId ?? piece.appearanceId}`
  const state = t(owned ? 'collections.pieceOwned' : 'collections.pieceMissing')
  const title = [`${name} (${state})`, source].filter(Boolean).join('\n')

  return (
    <li>
      <button
        type="button"
        title={title}
        aria-label={title}
        disabled={itemId === undefined}
        onClick={() => itemId !== undefined && openUrl(wowheadUrl(itemId, i18n.language))}
        className="block rounded-md transition-transform hover:scale-110 focus-visible:outline-2 focus-visible:outline-primary"
      >
        <ItemIcon item={item} className={cn('size-9', !owned && 'opacity-35 grayscale')} />
      </button>
    </li>
  )
}
