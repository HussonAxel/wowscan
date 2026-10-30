import type { TFunction } from 'i18next'

import { localized } from './format'
import type { TargetOption } from './targets'
import type { LocalizedString } from './types'

export function labelsText(labels: number[], texts: Record<number, LocalizedString>, locale: string): string {
  return labels
    .map((l) => localized(texts[l], locale))
    .filter(Boolean)
    .join(' · ')
}

/** Libellé court d'une option d'achat : « Mythique », « Normal », « Toutes sources »… */
export function optionLabel(
  option: TargetOption,
  kind: 'item' | 'appearance',
  texts: Record<number, LocalizedString>,
  locale: string,
  t: TFunction,
): string {
  if (kind === 'appearance') return t('versions.allSources')
  return labelsText(option.labels, texts, locale) || t('versions.normal')
}
