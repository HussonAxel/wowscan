import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import {
  CircleCheck,
  Coins,
  Download,
  ExternalLink,
  FolderSearch,
  Gem,
  ListPlus,
  RefreshCw,
  ScanSearch,
  Search,
  Skull,
} from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'

import { ArcScope } from '@/components/arc-scope'
import { EmptyState } from '@/components/arc/empty-state/empty-state'
import { Progress } from '@/components/arc/progress/progress'
import { ItemIcon } from '@/components/item-icon'
import { Money } from '@/components/money'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { startScan, useScan } from '@/features/scan/scan-store'
import { useVariantLabels } from '@/features/scan/use-scan-data'
import { addTargets, setTargets, useDebounced, useSetMeta } from '@/features/sets/set-helpers'
import { api } from '@/lib/api'
import { useErrorMessage } from '@/lib/errors'
import { localized, qualityColor, realmName, useApiLocale } from '@/lib/format'
import { labelsText } from '@/lib/labels'
import { openUrl } from '@/lib/open-url'
import { useSettings } from '@/lib/settings'
import type { CollectionStatus, Copper, ItemInfo, ScanResult, SetCollection, SetPiece } from '@/lib/types'
import { cn } from '@/lib/utils'

const PAGE = 50

type StatusFilter = 'inProgress' | 'incomplete' | 'complete' | 'all'

/** Meilleure offre de l'hôtel des ventes pour une apparence. */
interface Offer {
  price: Copper
  itemId: number
  /** `null` : objet empilable, même prix partout. */
  realmId: number | null
}

/** Offre la moins chère par apparence, tous objets et royaumes confondus. */
function appearanceOffers(result: ScanResult | null): Map<number, Offer> {
  const offers = new Map<number, Offer>()
  for (const item of result?.items ?? []) {
    for (const variant of item.variants) {
      if (variant.appearanceId == null) continue
      const best = variant.commodity
        ? { price: variant.commodity.min, realmId: null }
        : variant.realms[0] && { price: variant.realms[0].stats.min, realmId: variant.realms[0].connectedRealmId }
      const known = offers.get(variant.appearanceId)
      if (best && (!known || best.price < known.price)) offers.set(variant.appearanceId, { ...best, itemId: item.itemId })
    }
  }
  return offers
}

function setCost(set: SetCollection, offers: Map<number, Offer>) {
  const priced = set.missing.flatMap((a) => offers.get(a) ?? [])
  return { buyable: priced.length, cost: priced.reduce((sum, o) => sum + o.price, 0) }
}

function wowheadUrl(itemId: number, language: string) {
  return `https://www.wowhead.com/${language.startsWith('fr') ? 'fr/' : ''}item=${itemId}`
}

export function CollectionsPage() {
  const { t } = useTranslation()
  const { gameDir } = useSettings()
  const status = useQuery({
    queryKey: ['collection', gameDir],
    queryFn: () => api.collectionStatus(gameDir || null),
    // L'export change à chaque /reload en jeu : on relit en revenant sur la fenêtre.
    staleTime: 0,
    refetchOnWindowFocus: true,
  })
  const ready = Boolean(status.data?.gameDir && status.data.scannedAt)

  return (
    <div className="mx-auto w-full max-w-7xl space-y-6 p-6">
      <header className="space-y-1">
        <h1 className="text-2xl font-semibold tracking-tight">{t('collections.title')}</h1>
        <p className="max-w-2xl text-sm text-muted-foreground">{t('collections.subtitle')}</p>
      </header>

      {status.isPending ? (
        <Skeleton className="h-24 rounded-2xl" />
      ) : (
        <AddonCard status={status.data} refreshing={status.isFetching} onRefresh={() => void status.refetch()} />
      )}

      {ready && status.data ? (
        <CollectionSets status={status.data} />
      ) : (
        !status.isPending && (
          <ArcScope className="rounded-2xl bg-card ring-1 ring-foreground/8">
            <EmptyState icon={<Gem />} title={t('collections.emptyTitle')} description={t('collections.emptyDescription')} />
          </ArcScope>
        )
      )}
    </div>
  )
}

function AddonCard({
  status,
  refreshing,
  onRefresh,
}: {
  status: CollectionStatus | undefined
  refreshing: boolean
  onRefresh: () => void
}) {
  const { t, i18n } = useTranslation()
  const { gameDir } = useSettings()
  const errorMessage = useErrorMessage()
  const queryClient = useQueryClient()
  const classes = t('classes', { returnObjects: true }) as string[]

  const install = useMutation({
    mutationFn: () => api.installAddon(gameDir || null),
    onSuccess: (next) => {
      queryClient.setQueryData(['collection', gameDir], next)
      toast.success(t('collections.addon.installed'), { description: t('collections.addon.reloadHint') })
    },
    onError: (error) => toast.error(errorMessage(error)),
  })

  if (!status?.gameDir) {
    return (
      <section className="flex flex-wrap items-center gap-4 rounded-2xl bg-card p-5 ring-1 ring-foreground/8">
        <FolderSearch className="size-5 text-muted-foreground" />
        <div className="min-w-0 flex-1 space-y-1">
          <h2 className="font-semibold tracking-tight">{t('collections.game.notFound')}</h2>
          <p className="text-sm text-muted-foreground">{t('collections.game.notFoundDescription')}</p>
        </div>
        <Button variant="outline" asChild>
          <Link to="/settings">{t('collections.game.configure')}</Link>
        </Button>
      </section>
    )
  }

  const addonAction =
    status.addon !== 'installed' ? (
      <Button onClick={() => install.mutate()} disabled={install.isPending}>
        <Download />
        {status.addon === 'outdated' ? t('collections.addon.update') : t('collections.addon.install')}
      </Button>
    ) : null

  return (
    <section className="space-y-3 rounded-2xl bg-card p-5 ring-1 ring-foreground/8">
      <div className="flex flex-wrap items-start gap-4">
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="font-semibold tracking-tight">{t('collections.addon.title')}</h2>
            <Badge
              variant="outline"
              className={status.addon === 'installed' ? 'border-primary/30 text-primary' : 'text-muted-foreground'}
            >
              {status.addon === 'installed' && <CircleCheck />}
              {t(`collections.addon.${status.addon ?? 'missing'}`)}
            </Badge>
          </div>
          <p className="truncate font-mono text-xs text-muted-foreground" title={status.gameDir}>
            {status.gameDir}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          {addonAction}
          <Button variant="ghost" onClick={onRefresh} disabled={refreshing}>
            <RefreshCw className={cn(refreshing && 'animate-spin')} />
            {t('collections.refresh')}
          </Button>
        </div>
      </div>
      {status.scannedAt ? (
        <div className="space-y-1 text-sm">
          <p>
            {t('collections.exported', {
              count: status.appearances,
              date: new Date(status.scannedAt * 1000).toLocaleString(i18n.language, { dateStyle: 'medium', timeStyle: 'short' }),
            })}
          </p>
          {status.characters.length > 0 && (
            <p className="text-muted-foreground">
              {status.characters.map((c) => `${c.name} (${classes[c.classId] ?? '?'})`).join(' · ')}
            </p>
          )}
          <p className="text-xs text-muted-foreground">{t('collections.addon.reloadHint')}</p>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">
          {status.addon === 'installed' ? t('collections.addon.waiting') : t('collections.addon.why')}
        </p>
      )}
    </section>
  )
}

function CollectionSets({ status }: { status: CollectionStatus }) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const { gameDir } = useSettings()
  const errorMessage = useErrorMessage()
  const scan = useScan()
  const expansions = t('expansions', { returnObjects: true }) as string[]
  const classes = t('classes', { returnObjects: true }) as string[]

  const [query, setQuery] = useState('')
  const [classFilter, setClassFilter] = useState(status.characters.length ? 'mine' : 'all')
  const [expansion, setExpansion] = useState('all')
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('inProgress')
  const [shown, setShown] = useState(PAGE)
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const search = useDebounced(query.trim().toLocaleLowerCase(), 200)

  const sets = useQuery({
    queryKey: ['collection-sets', gameDir, status.scannedAt],
    queryFn: () => api.collectionSets(gameDir || null),
  })
  const credentials = useQuery({ queryKey: ['credentials'], queryFn: api.hasCredentials })
  const offers = useMemo(() => appearanceOffers(scan.result), [scan.result])

  const filtered = useMemo(() => {
    const mine = new Set(status.characters.map((c) => c.classId))
    const classMatch = (mask: number) => {
      if (classFilter === 'all' || !mask) return true
      const wanted = classFilter === 'mine' ? [...mine] : [Number(classFilter)]
      return wanted.some((id) => mask & (1 << (id - 1)))
    }
    const statusMatch = (s: SetCollection) =>
      statusFilter === 'all' ||
      (statusFilter === 'complete' && s.collected === s.total) ||
      (statusFilter === 'incomplete' && s.collected < s.total) ||
      (statusFilter === 'inProgress' && s.collected > 0 && s.collected < s.total)
    return (sets.data ?? [])
      .filter(
        (s) =>
          statusMatch(s) &&
          classMatch(s.classMask) &&
          (expansion === 'all' || s.expansion === Number(expansion)) &&
          (!search || Object.values(s.name).some((n) => n.toLocaleLowerCase().includes(search))),
      )
      .sort(
        (a, b) =>
          a.total - a.collected - (b.total - b.collected) ||
          b.collected / b.total - a.collected / a.total ||
          localized(a.name, locale).localeCompare(localized(b.name, locale)),
      )
  }, [sets.data, status.characters, classFilter, statusFilter, expansion, search, locale])

  const missing = useMemo(() => [...new Set(filtered.flatMap((s) => s.missing))], [filtered])
  const totals = useMemo(() => {
    let cost = 0
    let buyable = 0
    for (const appearance of missing) {
      const offer = offers.get(appearance)
      if (offer) [cost, buyable] = [cost + offer.price, buyable + 1]
    }
    return { cost, buyable }
  }, [missing, offers])

  async function onScanMissing() {
    try {
      const items = await api.appearanceItems(missing)
      await startScan([...new Set(Object.values(items).flat())])
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  if (sets.isPending) return <p className="text-sm text-muted-foreground">{t('sets.loading')}</p>
  if (sets.isError) return <p className="text-sm text-destructive">{errorMessage(sets.error)}</p>

  const visible = filtered.slice(0, shown)
  const scanning = scan.status === 'running'

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative min-w-56 flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t('sets.search')} className="pl-9" />
        </div>
        <Select value={statusFilter} onValueChange={(v) => setStatusFilter(v as StatusFilter)}>
          <SelectTrigger className="w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {(['inProgress', 'incomplete', 'complete', 'all'] as const).map((s) => (
              <SelectItem key={s} value={s}>
                {t(`collections.status.${s}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={classFilter} onValueChange={setClassFilter}>
          <SelectTrigger className="w-48">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {status.characters.length > 0 && <SelectItem value="mine">{t('collections.myClasses')}</SelectItem>}
            <SelectItem value="all">{t('sets.allClasses')}</SelectItem>
            {classes.map((name, id) =>
              name ? (
                <SelectItem key={id} value={String(id)}>
                  {name}
                </SelectItem>
              ) : null,
            )}
          </SelectContent>
        </Select>
        <Select value={expansion} onValueChange={setExpansion}>
          <SelectTrigger className="w-52">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t('collections.allExpansions')}</SelectItem>
            {expansions.map((name, id) => (
              <SelectItem key={id} value={String(id)}>
                {name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <section className="flex flex-wrap items-center gap-x-6 gap-y-3 rounded-2xl bg-card px-5 py-4 ring-1 ring-foreground/8">
        <Stat label={t('collections.stats.sets')} value={filtered.length} />
        <Stat label={t('collections.stats.missing')} value={missing.length} />
        {offers.size > 0 && (
          <>
            <Stat label={t('collections.stats.buyable')} value={totals.buyable} />
            <div className="space-y-0.5">
              <div className="text-xs text-muted-foreground">{t('collections.stats.cost')}</div>
              <Money copper={totals.cost} className="text-lg font-semibold" />
            </div>
          </>
        )}
        <div className="ml-auto flex flex-col items-end gap-1">
          <Button onClick={() => void onScanMissing()} disabled={scanning || !missing.length || credentials.data === false}>
            <ScanSearch />
            {scanning
              ? t('collections.scanning', { percent: scan.total ? Math.round((scan.done / scan.total) * 100) : 0 })
              : t('collections.scanMissing', { count: missing.length })}
          </Button>
          {credentials.data === false && (
            <Link to="/settings" className="text-xs text-muted-foreground underline-offset-2 hover:underline">
              {t('collections.needCredentials')}
            </Link>
          )}
        </div>
      </section>

      {filtered.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('collections.noSets')}</p>
      ) : (
        <div className="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.15fr)]">
          <div className="space-y-2">
            <ul className="space-y-2">
              {visible.map((set) => (
                <SetRow
                  key={set.id}
                  set={set}
                  offers={offers}
                  selected={set.id === selectedId}
                  onSelect={() => setSelectedId(set.id)}
                />
              ))}
            </ul>
            {filtered.length > shown && (
              <Button variant="ghost" className="w-full" onClick={() => setShown(shown + PAGE)}>
                {t('collections.showMore', { count: filtered.length - shown })}
              </Button>
            )}
          </div>
          <div className="lg:sticky lg:top-6">
            <AnimatePresence mode="wait">
              {selectedId != null ? (
                <motion.div
                  key={selectedId}
                  initial={{ opacity: 0, y: 6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -4 }}
                  transition={{ duration: 0.18, ease: 'easeOut' }}
                >
                  <CollectionPanel
                    set={filtered.find((s) => s.id === selectedId) ?? sets.data.find((s) => s.id === selectedId)!}
                    offers={offers}
                    scan={scan.result}
                  />
                </motion.div>
              ) : (
                <p className="rounded-2xl border border-dashed border-foreground/10 p-8 text-center text-sm text-muted-foreground">
                  {t('collections.selectHint')}
                </p>
              )}
            </AnimatePresence>
          </div>
        </div>
      )}
    </div>
  )
}

function Stat({ label, value }: { label: string; value: number }) {
  const { i18n } = useTranslation()
  return (
    <div className="space-y-0.5">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="text-lg font-semibold tabular-nums">{value.toLocaleString(i18n.language)}</div>
    </div>
  )
}

function SetRow({
  set,
  offers,
  selected,
  onSelect,
}: {
  set: SetCollection
  offers: Map<number, Offer>
  selected: boolean
  onSelect: () => void
}) {
  const locale = useApiLocale()
  const labelTexts = useVariantLabels()
  const meta = useSetMeta()
  const label = set.label != null ? labelsText([set.label], labelTexts, locale) : ''
  const { buyable, cost } = setCost(set, offers)
  const complete = set.collected === set.total

  return (
    <li>
      <button
        type="button"
        onClick={onSelect}
        aria-pressed={selected}
        className={cn(
          'w-full space-y-2 rounded-xl px-4 py-3 text-left ring-1 transition-colors duration-150',
          selected ? 'bg-primary/[0.07] ring-primary/40' : 'bg-card ring-foreground/8 hover:bg-foreground/[0.03]',
        )}
      >
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="truncate font-medium">{localized(set.name, locale)}</span>
              {label && (
                <Badge variant="outline" className="shrink-0 border-primary/30 text-primary">
                  {label}
                </Badge>
              )}
            </div>
            <p className="truncate text-xs text-muted-foreground">{meta(set)}</p>
          </div>
          <div className="shrink-0 text-right text-xs">
            <div className={cn('font-medium tabular-nums', complete && 'text-primary')}>
              {set.collected} / {set.total}
            </div>
            {buyable > 0 && (
              <div className="flex items-center justify-end gap-1 text-muted-foreground">
                <Coins className="size-3" />
                <Money copper={cost} />
              </div>
            )}
          </div>
        </div>
        <div className="h-1 overflow-hidden rounded-full bg-foreground/[0.08]">
          <div
            className={cn('h-full rounded-full', complete ? 'bg-primary' : 'bg-primary/60')}
            style={{ width: `${(set.collected / Math.max(set.total, 1)) * 100}%` }}
          />
        </div>
      </button>
    </li>
  )
}

function CollectionPanel({ set, offers, scan }: { set: SetCollection; offers: Map<number, Offer>; scan: ScanResult | null }) {
  const { t, i18n } = useTranslation()
  const locale = useApiLocale()
  const errorMessage = useErrorMessage()
  const labelTexts = useVariantLabels()
  const meta = useSetMeta()

  const details = useQuery({ queryKey: ['set', set.id], queryFn: () => api.setDetails(set.id) })
  const data = details.data
  const pieceItems = data?.pieces.map((p) => p.setSources[0].itemId) ?? []
  const infos = useQuery({
    queryKey: ['items', pieceItems],
    queryFn: () => api.itemInfos(pieceItems),
    enabled: pieceItems.length > 0,
  })

  if (!data) {
    return (
      <div className="space-y-3 rounded-2xl bg-card p-5 ring-1 ring-foreground/8">
        <Skeleton className="h-6 w-2/3" />
        <Skeleton className="h-4 w-1/3" />
        {Array.from({ length: 5 }, (_, i) => (
          <Skeleton key={i} className="h-16 rounded-xl" />
        ))}
      </div>
    )
  }

  const missingSet = new Set(set.missing)
  const missingPieces = data.pieces.filter((p) => missingSet.has(p.appearanceId))
  const ownedPieces = data.pieces.filter((p) => !missingSet.has(p.appearanceId))
  const label = set.label != null ? labelsText([set.label], labelTexts, locale) : ''
  const { buyable, cost } = setCost(set, offers)

  function onAdd() {
    const targets = setTargets(data!).filter((target) => target.kind === 'appearance' && missingSet.has(target.appearanceId))
    const count = addTargets(targets)
    toast.success(t('sets.added', { count: count || targets.length }))
  }

  async function onScan() {
    try {
      const items = await api.appearanceItems(set.missing)
      await startScan([...new Set(Object.values(items).flat())])
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <section className="space-y-4 rounded-2xl bg-card p-5 ring-1 ring-foreground/8">
      <div className="space-y-1">
        <div className="flex flex-wrap items-center gap-2">
          <h2 className="text-lg font-semibold tracking-tight">{localized(set.name, locale)}</h2>
          {label && (
            <Badge variant="outline" className="border-primary/30 text-primary">
              {label}
            </Badge>
          )}
        </div>
        <p className="text-sm text-muted-foreground">{meta(set)}</p>
      </div>

      <ArcScope>
        <Progress value={set.collected} max={Math.max(set.total, 1)} label={t('collections.progress', { collected: set.collected, total: set.total })} showValue />
      </ArcScope>

      {missingPieces.length > 0 && (
        <div className="flex flex-wrap items-center gap-2">
          <Button onClick={() => void onScan()}>
            <ScanSearch />
            {t('collections.scanSet')}
          </Button>
          <Button variant="outline" onClick={onAdd}>
            <ListPlus />
            {t('collections.addMissing')}
          </Button>
          {buyable > 0 && (
            <span className="ml-auto text-sm text-muted-foreground">
              {t('collections.setCost', { count: buyable })} <Money copper={cost} className="font-medium text-foreground" />
            </span>
          )}
        </div>
      )}

      {missingPieces.length > 0 && (
        <div className="space-y-2">
          <h3 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
            {t('collections.missingPieces', { count: missingPieces.length })}
          </h3>
          <ul className="space-y-2">
            {missingPieces.map((piece) => (
              <MissingPiece
                key={piece.appearanceId}
                piece={piece}
                info={infos.data?.[piece.setSources[0].itemId]}
                offer={offers.get(piece.appearanceId)}
                scan={scan}
                language={i18n.language}
              />
            ))}
          </ul>
        </div>
      )}

      {ownedPieces.length > 0 && (
        <div className="space-y-2">
          <h3 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
            {t('collections.ownedPieces', { count: ownedPieces.length })}
          </h3>
          <ul className="grid gap-1.5 sm:grid-cols-2">
            {ownedPieces.map((piece) => {
              const info = infos.data?.[piece.setSources[0].itemId]
              return (
                <li key={piece.appearanceId} className="flex items-center gap-2 rounded-lg px-2 py-1.5 text-sm text-muted-foreground">
                  <CircleCheck className="size-4 shrink-0 text-primary" />
                  <span className="truncate">
                    {info ? localized(info.name, locale) : t(`slots.${piece.slot ?? 0}`, { defaultValue: `#${piece.setSources[0].itemId}` })}
                  </span>
                </li>
              )
            })}
          </ul>
        </div>
      )}
    </section>
  )
}

function MissingPiece({
  piece,
  info,
  offer,
  scan,
  language,
}: {
  piece: SetPiece
  info: ItemInfo | undefined
  offer: Offer | undefined
  scan: ScanResult | null
  language: string
}) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const itemId = piece.setSources[0].itemId
  const item = info
  const kinds = [...new Set(piece.allSources.map((s) => s.kind))].filter((k) => k > 0 && k !== 9).sort((a, b) => a - b)
  const drops = [...new Map(piece.allSources.flatMap((s) => s.drops).map((d) => [d.encounterId, d])).values()]
  const slot = piece.slot ? t(`slots.${piece.slot}`, { defaultValue: '' }) : ''
  const realm = offer?.realmId != null ? scan?.realms.find((r) => r.id === offer.realmId) : undefined

  return (
    <li className="flex gap-3 rounded-xl bg-foreground/[0.025] p-3 ring-1 ring-foreground/6">
      <ItemIcon item={item} />
      <div className="min-w-0 flex-1 space-y-1.5">
        <div className="flex items-start gap-2">
          <div className="min-w-0 flex-1">
            <div className={cn('truncate text-sm font-medium', qualityColor[item?.quality ?? ''])}>
              {item ? localized(item.name, locale) : slot || `#${itemId}`}
            </div>
            <div className="truncate text-xs text-muted-foreground">
              {[item && slot, ...kinds.map((k) => t(`collections.kinds.${k}`, { defaultValue: '' }))].filter(Boolean).join(' · ')}
            </div>
          </div>
          <Button variant="ghost" size="icon-xs" title={t('collections.wowhead')} onClick={() => openUrl(wowheadUrl(itemId, language))}>
            <ExternalLink />
          </Button>
        </div>
        {drops.length > 0 && (
          <ul className="space-y-0.5 text-xs">
            {drops.slice(0, 3).map((d) => (
              <li key={d.encounterId} className="flex items-center gap-1.5">
                <Skull className="size-3 shrink-0 text-muted-foreground" />
                <span className="truncate">
                  <span className="font-medium">{localized(d.encounter, locale)}</span>
                  <span className="text-muted-foreground"> — {localized(d.instance, locale)}</span>
                </span>
              </li>
            ))}
          </ul>
        )}
        {kinds.includes(3) && <p className="text-xs text-muted-foreground">{t('collections.vendorHint')}</p>}
        {offer && (
          <div className="flex flex-wrap items-center gap-x-2 text-xs">
            <Coins className="size-3 text-muted-foreground" />
            <Money copper={offer.price} className="font-medium" />
            <span className="truncate text-muted-foreground">
              {offer.realmId == null ? t('results.commodity') : realmName(realm, locale)}
            </span>
          </div>
        )}
      </div>
    </li>
  )
}
