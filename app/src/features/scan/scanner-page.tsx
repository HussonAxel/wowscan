import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { ClipboardPaste, KeyRound, ScanSearch, Trash2, TriangleAlert } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'

import { ArcScope } from '@/components/arc-scope'
import { EmptyState } from '@/components/arc/empty-state/empty-state'
import { Button } from '@/components/ui/button'
import { api } from '@/lib/api'
import { useItemInfos } from '@/lib/item-infos'
import { useErrorMessage } from '@/lib/errors'
import { localized, useApiLocale } from '@/lib/format'
import { optionLabel } from '@/lib/labels'
import { settings, useSettings } from '@/lib/settings'
import { targetItemIds, targetKey, targetOptions } from '@/lib/targets'
import { defaultOption } from '@/lib/versions'

import { importText } from './import-text'
import { ItemListInput } from './item-list-input'
import { PurchasePlan, type PlanLine } from './purchase-plan'
import { ScanProgress } from './scan-progress'
import { startScan, useScan } from './scan-store'
import { TargetCard, type TargetView } from './target-card'
import { useRequestedVariants, useVariantLabels } from './use-scan-data'

export function ScannerPage() {
  const { t, i18n } = useTranslation()
  const locale = useApiLocale()
  const errorMessage = useErrorMessage()
  const { targets, ownedRealms, preferredVersion } = useSettings()
  const scan = useScan()
  const [selected, setSelected] = useState<Record<string, string>>({})

  const credentials = useQuery({ queryKey: ['credentials'], queryFn: api.hasCredentials })
  const ready = credentials.data === true
  const labelTexts = useVariantLabels()
  const requested = useRequestedVariants(targets)

  // Cibles couvertes par le dernier scan, et leurs options d'achat.
  const views = useMemo<TargetView[]>(() => {
    const result = scan.result
    if (!result) return []
    const scanned = new Set(result.items.map((i) => i.itemId))
    return targets
      .filter((target) => targetItemIds(target).every((id) => scanned.has(id)))
      .map((target) => {
        const key = targetKey(target)
        const options = targetOptions(target, result)
        const requestedAppearance = requested[key]?.appearanceId ?? null
        const chosen =
          options.find((o) => o.key === selected[key]) ??
          defaultOption(options, requestedAppearance, preferredVersion, labelTexts)
        return { key, target, options, chosen, requestedAppearance }
      })
  }, [scan.result, targets, requested, selected, preferredVersion, labelTexts])
  const pending = targets.length - views.length

  // Fiches : objets saisis, plus les objets à acheter pour les pièces d'ensemble.
  const infoIds = useMemo(() => {
    const ids = new Set(targets.map((t) => t.itemId))
    for (const view of views) for (const option of view.options) for (const r of option.realms.slice(0, 12)) ids.add(r.itemId)
    return [...ids].sort((a, b) => a - b)
  }, [targets, views])
  const infoMap = useItemInfos(infoIds, ready)

  const realms = useMemo(() => new Map(scan.result?.realms.map((r) => [r.id, r]) ?? []), [scan.result])
  const owned = useMemo(() => new Set(ownedRealms), [ownedRealms])

  const lines: PlanLine[] = views.flatMap((view) =>
    view.chosen
      ? [
          {
            key: view.key,
            title: infoMap[view.target.itemId] ? localized(infoMap[view.target.itemId].name, locale) : `#${view.target.itemId}`,
            // La version n'est utile que s'il y avait un choix à faire.
            version: view.options.length > 1 ? optionLabel(view.chosen, view.target.kind, labelTexts, locale, t) : '',
            info: infoMap[view.target.itemId],
            option: view.chosen,
          },
        ]
      : [],
  )

  // Coller n'importe où sur la page (hors champ de saisie) importe le texte.
  useEffect(() => {
    function onPaste(event: ClipboardEvent) {
      const field = (event.target as HTMLElement | null)?.closest('input, textarea, [contenteditable]')
      const text = event.clipboardData?.getData('text') ?? ''
      if (field || !text.trim()) return
      event.preventDefault()
      void importText(text, t).then((count) => count && toast.success(t('scan.added', { count })))
    }
    window.addEventListener('paste', onPaste)
    return () => window.removeEventListener('paste', onPaste)
  }, [t])

  async function onScan() {
    try {
      const ids = [...new Set(targets.flatMap(targetItemIds))]
      const result = await startScan(ids)
      if (!result) return
      toast.success(t('scan.done'), {
        description: t('scan.doneDescription', { items: targets.length, realms: result.realms.length }),
      })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const running = scan.status === 'running'

  return (
    <div className="mx-auto w-full max-w-7xl space-y-6 p-6">
      <header className="space-y-1">
        <h1 className="text-2xl font-semibold tracking-tight">{t('scan.title')}</h1>
        <p className="max-w-2xl text-sm text-muted-foreground">{t('scan.subtitle')}</p>
      </header>

      {credentials.data === false ? (
        <ArcScope className="rounded-2xl bg-card ring-1 ring-foreground/8">
          <EmptyState
            icon={<KeyRound />}
            title={t('scan.noCredentialsTitle')}
            description={t('scan.noCredentialsDescription')}
            action={
              <Button asChild>
                <Link to="/settings">{t('scan.goToSettings')}</Link>
              </Button>
            }
          />
        </ArcScope>
      ) : (
        <>
          <div className="space-y-3">
            <ItemListInput targets={targets} infos={infoMap} requested={requested} labelTexts={labelTexts} />
            <div className="flex flex-wrap items-center gap-2">
              <Button onClick={() => void onScan()} disabled={running || !ready || targets.length === 0}>
                <ScanSearch />
                {running ? t('scan.scanning') : t('scan.start')}
              </Button>
              {targets.length > 0 && (
                <Button variant="ghost" disabled={running} onClick={() => settings.update({ targets: [] })}>
                  <Trash2 />
                  {t('scan.clear')}
                </Button>
              )}
              <span className="ml-auto text-xs text-muted-foreground">
                {scan.result
                  ? t('scan.scannedAt', {
                      time: new Date(scan.result.scannedAt * 1000).toLocaleTimeString(i18n.language, {
                        hour: '2-digit',
                        minute: '2-digit',
                      }),
                    })
                  : null}
              </span>
            </div>
            <ScanProgress />
          </div>

          {views.length > 0 ? (
            <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_380px]">
              <div className="space-y-4">
                {scan.result && scan.result.failures.length > 0 && (
                  <p className="flex items-center gap-2 rounded-xl bg-destructive/10 px-3 py-2 text-sm text-destructive ring-1 ring-destructive/20">
                    <TriangleAlert className="size-4" />
                    {t('scan.failures', { count: scan.result.failures.length })}
                  </p>
                )}
                {pending > 0 && !running && (
                  <p className="rounded-xl bg-foreground/[0.03] px-3 py-2 text-sm text-muted-foreground ring-1 ring-foreground/8">
                    {t('scan.pending', { count: pending })}
                  </p>
                )}
                {views.map((view) => (
                  <TargetCard
                    key={view.key}
                    view={view}
                    infos={infoMap}
                    labelTexts={labelTexts}
                    realms={realms}
                    owned={owned}
                    onSelect={(optionKey) => setSelected((s) => ({ ...s, [view.key]: optionKey }))}
                  />
                ))}
              </div>
              <div className="xl:sticky xl:top-6">
                <PurchasePlan lines={lines} realms={realms} />
              </div>
            </div>
          ) : (
            !running && (
              <ArcScope className="rounded-2xl bg-card ring-1 ring-foreground/8">
                <EmptyState
                  icon={targets.length ? <ScanSearch /> : <ClipboardPaste />}
                  title={t('scan.emptyTitle')}
                  description={t('scan.emptyDescription')}
                />
              </ArcScope>
            )
          )}
        </>
      )}
    </div>
  )
}
