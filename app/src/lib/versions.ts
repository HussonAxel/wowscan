import type { PreferredVersion } from './settings'
import type { TargetOption } from './targets'
import type { LocalizedString } from './types'

/**
 * Difficulté d'une version, d'après ses libellés en anglais (« Mythic », « Heroic »,
 * « Raid Finder »). Sans aucun de ces libellés, c'est la version normale.
 */
export function difficultyOf(labels: number[], texts: Record<number, LocalizedString>): Exclude<PreferredVersion, 'cheapest'> {
  const english = labels.map((l) => texts[l]?.en_GB ?? texts[l]?.en_US ?? '').join(' ')
  if (/\bMythic\b/.test(english)) return 'mythic'
  if (/\bHeroic\b/.test(english)) return 'heroic'
  if (/\bRaid Finder\b/.test(english)) return 'raidFinder'
  return 'normal'
}

/**
 * Option présélectionnée : la version demandée (bonus du lien) si elle est en vente,
 * sinon la préférence de l'utilisateur, sinon la moins chère (les options sont triées).
 */
export function defaultOption(
  options: TargetOption[],
  requestedAppearance: number | null | undefined,
  preferred: PreferredVersion,
  texts: Record<number, LocalizedString>,
): TargetOption | undefined {
  if (requestedAppearance != null) {
    const requested = options.find((o) => o.appearanceId === requestedAppearance)
    if (requested) return requested
  }
  if (preferred !== 'cheapest') {
    const match = options.find((o) => difficultyOf(o.labels, texts) === preferred)
    if (match) return match
  }
  return options[0]
}
