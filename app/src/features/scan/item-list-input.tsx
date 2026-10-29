import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'

import { ArcScope } from '@/components/arc-scope'
import { TagInput } from '@/components/arc/tag-input/tag-input'
import { api } from '@/lib/api'
import { localized, useApiLocale } from '@/lib/format'
import { settings } from '@/lib/settings'
import type { ItemInfo } from '@/lib/types'

/**
 * Liste d'objets à scanner. Chaque saisie (identifiant, lien Wowhead, lien en jeu,
 * ou plusieurs à la fois) est convertie en identifiants ; l'étiquette affiche ensuite le nom.
 */
export function ItemListInput({ ids, infos }: { ids: number[]; infos: Record<number, ItemInfo> }) {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const labelFor = (id: number) => (infos[id] ? `${localized(infos[id].name, locale)} · ${id}` : `#${id}`)
  const tags = ids.map(labelFor)

  async function onValueChange(next: string[]) {
    const kept = ids.filter((id) => next.includes(labelFor(id)))
    const typed = next.filter((tag) => !tags.includes(tag))
    let added: number[] = []
    if (typed.length) {
      const parsed = await api.parseItems(typed.join(' '))
      added = parsed.itemIds
      if (parsed.unrecognized.length) {
        toast.warning(t('scan.unrecognized', { tokens: parsed.unrecognized.join(', ') }))
      }
    }
    settings.update({ itemIds: [...new Set([...kept, ...added])] })
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
