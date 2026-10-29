import { useTranslation } from 'react-i18next'

import { ArcScope } from '@/components/arc-scope'
import { Progress } from '@/components/arc/progress/progress'
import { TextShimmer } from '@/components/motion/text-shimmer'

import { useScan } from './scan-store'

/** Barre de progression déterminée, avec le compte des marchés lus. */
export function ScanProgress() {
  const { t } = useTranslation()
  const scan = useScan()
  if (scan.status !== 'running') return null

  return (
    <div className="space-y-2 rounded-xl bg-foreground/[0.03] p-4 ring-1 ring-foreground/8">
      <div className="flex items-baseline justify-between gap-4 text-sm">
        <TextShimmer className="font-medium">{t('scan.scanning')}</TextShimmer>
        <span className="tabular-nums text-muted-foreground">
          {t('scan.progress', { done: scan.done, total: scan.total || '…' })}
          {scan.cached > 0 && <> · {t('scan.fromCache', { count: scan.cached })}</>}
        </span>
      </div>
      <ArcScope>
        <Progress value={scan.done} max={Math.max(scan.total, 1)} label={t('scan.scanning')} />
      </ArcScope>
    </div>
  )
}
