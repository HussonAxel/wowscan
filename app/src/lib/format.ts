import { useTranslation } from 'react-i18next'

import { apiLocale } from '@/i18n'

import type { ConnectedRealm, LocalizedString } from './types'

export function localized(text: LocalizedString | undefined, locale: string): string {
  if (!text) return ''
  return text[locale] ?? text.en_GB ?? text.en_US ?? Object.values(text)[0] ?? ''
}

export function realmName(realm: ConnectedRealm | undefined, locale: string): string {
  if (!realm) return '?'
  return realm.realms
    .map((r) => localized(r.name, locale))
    .sort((a, b) => a.localeCompare(b))
    .join(', ')
}

/** Locale API correspondant à la langue de l'interface. */
export function useApiLocale() {
  const { i18n } = useTranslation()
  return apiLocale(i18n.language)
}

export const qualityColor: Record<string, string> = {
  POOR: 'text-q-poor',
  COMMON: 'text-q-common',
  UNCOMMON: 'text-q-uncommon',
  RARE: 'text-q-rare',
  EPIC: 'text-q-epic',
  LEGENDARY: 'text-q-legendary',
  ARTIFACT: 'text-q-artifact',
  HEIRLOOM: 'text-q-heirloom',
}

export const qualityBorder: Record<string, string> = {
  POOR: 'ring-q-poor/50',
  COMMON: 'ring-q-common/30',
  UNCOMMON: 'ring-q-uncommon/50',
  RARE: 'ring-q-rare/60',
  EPIC: 'ring-q-epic/60',
  LEGENDARY: 'ring-q-legendary/60',
  ARTIFACT: 'ring-q-artifact/60',
  HEIRLOOM: 'ring-q-heirloom/60',
}
