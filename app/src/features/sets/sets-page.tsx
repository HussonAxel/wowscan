import { useQuery } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { ListPlus, ScanSearch, Search, Shirt } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'

import { ArcScope } from '@/components/arc-scope'
import { EmptyState } from '@/components/arc/empty-state/empty-state'
import { ItemIcon } from '@/components/item-icon'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { startScan } from '@/features/scan/scan-store'
import { useVariantLabels } from '@/features/scan/use-scan-data'
import { api } from '@/lib/api'
import { useItemInfos } from '@/lib/item-infos'
import { useErrorMessage } from '@/lib/errors'
import { localized, qualityColor, useApiLocale } from '@/lib/format'
import { labelsText } from '@/lib/labels'
import { settings } from '@/lib/settings'
import { targetItemIds } from '@/lib/targets'
import type { SetSummary } from '@/lib/types'
import { cn } from '@/lib/utils'

import { addTargets, setTargets, useDebounced, useSetMeta } from './set-helpers'

export function SetsPage() {
  const { t } = useTranslation()
  const [query, setQuery] = useState('')
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const search = useDebounced(query.trim(), 200)

  const results = useQuery({
    queryKey: ['sets', search],
    queryFn: () => api.searchSets(search),
    enabled: search.length >= 2,
    placeholderData: (previous) => previous,
  })
  const sets = search.length >= 2 ? (results.data ?? []) : []

  return (
    <div className="mx-auto w-full max-w-7xl space-y-6 p-6">
      <header className="space-y-1">
        <h1 className="text-2xl font-semibold tracking-tight">{t('sets.title')}</h1>
        <p className="max-w-2xl text-sm text-muted-foreground">{t('sets.subtitle')}</p>
      </header>

      <div className="relative">
        <Search className="pointer-events-none absolute top-1/2 left-3.5 size-4.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t('sets.search')}
          className="h-12 rounded-xl pl-10 text-base"
        />
        {sets.length > 0 && (
          <span className="absolute top-1/2 right-4 -translate-y-1/2 text-xs text-muted-foreground tabular-nums">
            {t('sets.results', { count: sets.length })}
          </span>
        )}
      </div>

      {search.length < 2 ? (
        <ArcScope className="rounded-2xl bg-card ring-1 ring-foreground/8">
          <EmptyState icon={<Shirt />} title={t('sets.emptyTitle')} description={t('sets.emptyDescription')} />
        </ArcScope>
      ) : results.isPending ? (
        <p className="text-sm text-muted-foreground">{t('sets.loading')}</p>
      ) : sets.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('sets.noResults', { query: search })}</p>
      ) : (
        <div className="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.15fr)]">
          <SetList sets={sets} selectedId={selectedId} onSelect={setSelectedId} />
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
                  <SetPanel id={selectedId} />
                </motion.div>
              ) : (
                <p className="rounded-2xl border border-dashed border-foreground/10 p-8 text-center text-sm text-muted-foreground">
                  {t('sets.selectHint')}
                </p>
              )}
            </AnimatePresence>
          </div>
        </div>
      )}
    </div>
  )
}

function SetList({
  sets,
  selectedId,
  onSelect,
}: {
  sets: SetSummary[]
  selectedId: number | null
  onSelect: (id: number) => void
}) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const labelTexts = useVariantLabels()
  const meta = useSetMeta()

  return (
    <ul className="space-y-2">
      {sets.map((set) => {
        const selected = set.id === selectedId
        const label = set.label != null ? labelsText([set.label], labelTexts, locale) : ''
        return (
          <li key={set.id}>
            <button
              type="button"
              onClick={() => onSelect(set.id)}
              aria-pressed={selected}
              className={cn(
                'flex w-full items-center gap-3 rounded-xl px-4 py-3 text-left ring-1 transition-colors duration-150',
                selected ? 'bg-primary/[0.07] ring-primary/40' : 'bg-card ring-foreground/8 hover:bg-foreground/[0.03]',
              )}
            >
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
              <span className="shrink-0 text-xs text-muted-foreground tabular-nums">
                {t('sets.pieces', { count: set.appearances })}
              </span>
            </button>
          </li>
        )
      })}
    </ul>
  )
}

function SetPanel({ id }: { id: number }) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const navigate = useNavigate()
  const errorMessage = useErrorMessage()
  const labelTexts = useVariantLabels()
  const meta = useSetMeta()

  const details = useQuery({ queryKey: ['set', id], queryFn: () => api.setDetails(id) })
  const set = details.data
  const pieceItems = set?.pieces.map((p) => p.setSources[0].itemId) ?? []
  const infos = useItemInfos(pieceItems)

  if (!set) {
    return (
      <div className="space-y-3 rounded-2xl bg-card p-5 ring-1 ring-foreground/8">
        <Skeleton className="h-6 w-2/3" />
        <Skeleton className="h-4 w-1/3" />
        <div className="grid gap-2 sm:grid-cols-2">
          {Array.from({ length: 8 }, (_, i) => (
            <Skeleton key={i} className="h-14 rounded-xl" />
          ))}
        </div>
      </div>
    )
  }

  const label = set.label != null ? labelsText([set.label], labelTexts, locale) : ''

  function onAdd() {
    const count = addTargets(setTargets(set!))
    toast.success(t('sets.added', { count: count || set!.pieces.length }))
  }

  async function onScan() {
    addTargets(setTargets(set!))
    void navigate({ to: '/' })
    try {
      await startScan([...new Set(settings.get().targets.flatMap(targetItemIds))])
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <section className="space-y-4 rounded-2xl bg-card p-5 ring-1 ring-foreground/8">
      <div className="flex items-start gap-4">
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-lg font-semibold tracking-tight">{localized(set.name, locale)}</h2>
            {label && (
              <Badge variant="outline" className="border-primary/30 text-primary">
                {label}
              </Badge>
            )}
          </div>
          <p className="text-sm text-muted-foreground">
            {meta(set)} · {t('sets.pieces', { count: set.pieces.length })}
          </p>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button onClick={() => void onScan()}>
          <ScanSearch />
          {t('sets.scan')}
        </Button>
        <Button variant="outline" onClick={onAdd}>
          <ListPlus />
          {t('sets.add')}
        </Button>
      </div>

      <ul className="grid gap-2 sm:grid-cols-2">
        {set.pieces.map((piece) => {
          const info = infos[piece.setSources[0].itemId]
          const sources = new Set(piece.allSources.map((s) => s.itemId)).size
          return (
            <li key={piece.appearanceId} className="flex items-center gap-3 rounded-xl bg-foreground/[0.025] p-2.5 ring-1 ring-foreground/6">
              <ItemIcon item={info} />
              <div className="min-w-0 flex-1">
                <div className={cn('truncate text-sm font-medium', qualityColor[info?.quality ?? ''])}>
                  {info ? localized(info.name, locale) : `#${piece.setSources[0].itemId}`}
                </div>
                <div className="truncate text-xs text-muted-foreground">
                  {[info && localized(info.inventoryType, locale), sources > 1 && t('sets.sources', { count: sources })]
                    .filter(Boolean)
                    .join(' · ')}
                </div>
              </div>
            </li>
          )
        })}
      </ul>
    </section>
  )
}
