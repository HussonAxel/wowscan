import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { KeyRound, ScanSearch, Trash2, TriangleAlert } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'

import { ArcScope } from '@/components/arc-scope'
import { EmptyState } from '@/components/arc/empty-state/empty-state'
import { Button } from '@/components/ui/button'
import { api } from '@/lib/api'
import { useErrorMessage } from '@/lib/errors'
import { settings, useSettings } from '@/lib/settings'

import { ItemListInput } from './item-list-input'
import { ItemResultCard } from './item-result-card'
import { PurchasePlan } from './purchase-plan'
import { ScanProgress } from './scan-progress'
import { startScan, useScan } from './scan-store'

export function ScannerPage() {
  const { t, i18n } = useTranslation()
  const errorMessage = useErrorMessage()
  const { itemIds, ownedRealms } = useSettings()
  const scan = useScan()

  const credentials = useQuery({ queryKey: ['credentials'], queryFn: api.hasCredentials })
  const ready = credentials.data === true

  // Fiches des objets saisis et des objets du dernier scan.
  const wantedInfos = useMemo(
    () => [...new Set([...itemIds, ...(scan.result?.items.map((i) => i.itemId) ?? [])])].sort((a, b) => a - b),
    [itemIds, scan.result],
  )
  const infos = useQuery({
    queryKey: ['items', wantedInfos],
    queryFn: () => api.itemInfos(wantedInfos),
    enabled: ready && wantedInfos.length > 0,
    placeholderData: (previous) => previous,
  })
  const infoMap = infos.data ?? {}

  const realms = useMemo(() => new Map(scan.result?.realms.map((r) => [r.id, r]) ?? []), [scan.result])
  const owned = useMemo(() => new Set(ownedRealms), [ownedRealms])

  async function onScan() {
    try {
      const result = await startScan(itemIds)
      if (!result) return
      toast.success(t('scan.done'), {
        description: t('scan.doneDescription', { items: result.items.length, realms: result.realms.length }),
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
        <ArcScope className="rounded-xl bg-card ring-1 ring-foreground/8">
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
            <ItemListInput ids={itemIds} infos={infoMap} />
            <div className="flex items-center gap-2">
              <Button onClick={() => void onScan()} disabled={running || !ready || itemIds.length === 0}>
                <ScanSearch />
                {running ? t('scan.scanning') : t('scan.start')}
              </Button>
              {itemIds.length > 0 && (
                <Button variant="ghost" disabled={running} onClick={() => settings.update({ itemIds: [] })}>
                  <Trash2 />
                  {t('scan.clear')}
                </Button>
              )}
              {scan.result && (
                <span className="ml-auto text-xs text-muted-foreground">
                  {t('scan.scannedAt', {
                    time: new Date(scan.result.scannedAt * 1000).toLocaleTimeString(i18n.language, {
                      hour: '2-digit',
                      minute: '2-digit',
                    }),
                  })}
                </span>
              )}
            </div>
            <ScanProgress />
          </div>

          {scan.result ? (
            <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,1fr)_380px]">
              <div className="space-y-4">
                {scan.result.failures.length > 0 && (
                  <p className="flex items-center gap-2 rounded-lg bg-destructive/10 px-3 py-2 text-sm text-destructive ring-1 ring-destructive/20">
                    <TriangleAlert className="size-4" />
                    {t('scan.failures', { count: scan.result.failures.length })}
                  </p>
                )}
                {scan.result.items.map((item) => (
                  <ItemResultCard
                    key={item.itemId}
                    result={item}
                    info={infoMap[item.itemId]}
                    realms={realms}
                    owned={owned}
                  />
                ))}
              </div>
              <div className="xl:sticky xl:top-6">
                <PurchasePlan result={scan.result} infos={infoMap} realms={realms} />
              </div>
            </div>
          ) : (
            !running && (
              <ArcScope className="rounded-xl bg-card ring-1 ring-foreground/8">
                <EmptyState icon={<ScanSearch />} title={t('scan.emptyTitle')} description={t('scan.emptyDescription')} />
              </ArcScope>
            )
          )}
        </>
      )}
    </div>
  )
}
