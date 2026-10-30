import { useTranslation } from 'react-i18next'

import { ArcScope } from '@/components/arc-scope'
import { TagInput } from '@/components/arc/tag-input/tag-input'
import { localized, useApiLocale } from '@/lib/format'
import { labelsText } from '@/lib/labels'
import { settings } from '@/lib/settings'
import { type Target, targetKey } from '@/lib/targets'
import type { ItemInfo, LocalizedString, Variant } from '@/lib/types'

import { importText } from './import-text'

/**
 * Liste des cibles du scanner. Chaque saisie (lien Wowhead, lien en jeu, identifiant,
 * ligne de chat entière) passe par `importText` ; l'étiquette affiche ensuite le nom
 * et la version demandée.
 */
export function ItemListInput({
  targets,
  infos,
  requested,
  labelTexts,
}: {
  targets: Target[]
  infos: Record<number, ItemInfo>
  requested: Record<string, Variant>
  labelTexts: Record<number, LocalizedString>
}) {
  const { t } = useTranslation()
  const locale = useApiLocale()

  const tags: string[] = []
  for (const target of targets) {
    const name = infos[target.itemId] ? localized(infos[target.itemId].name, locale) : `#${target.itemId}`
    // Une pièce d'ensemble garde le nom de l'objet : n'importe quelle source de l'apparence convient.
    const version =
      target.kind === 'item' ? labelsText(requested[targetKey(target)]?.labels ?? [], labelTexts, locale) : ''
    let tag = version ? `${name} · ${version}` : name
    // Étiquettes uniques : la liste les compare comme du texte.
    while (tags.includes(tag)) tag += ' ·'
    tags.push(tag)
  }

  async function onValueChange(next: string[]) {
    const kept = targets.filter((_, i) => next.includes(tags[i]))
    if (kept.length !== targets.length) settings.update({ targets: kept })
    // Chaque nouvelle saisie est lue seule : une faute de frappe n'invalide pas les autres.
    for (const typed of next.filter((tag) => !tags.includes(tag))) {
      await importText(typed, t)
    }
  }

  return (
    <ArcScope>
      <TagInput
        label={t('scan.inputLabel')}
        value={tags}
        onValueChange={(next) => void onValueChange(next)}
        placeholder={t('scan.inputPlaceholder')}
        description={t('scan.inputHint')}
      />
    </ArcScope>
  )
}
