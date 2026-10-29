import { useNavigate } from '@tanstack/react-router'
import { Languages, ScanSearch, Settings, Trash2, Zap } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { CommandMenu, type CommandMenuAction } from '@/components/ui/command-menu'
import { startScan } from '@/features/scan/scan-store'
import { settings } from '@/lib/settings'

/** Palette Ctrl+K : navigation et actions courantes au clavier. */
export function CommandPalette() {
  const { t } = useTranslation()
  const navigate = useNavigate()

  const actions: CommandMenuAction[] = [
    { id: 'go-scanner', label: t('nav.scanner'), group: t('command.navigation'), icon: <ScanSearch />, action: () => void navigate({ to: '/' }) },
    { id: 'go-settings', label: t('nav.settings'), group: t('command.navigation'), icon: <Settings />, action: () => void navigate({ to: '/settings' }) },
    {
      id: 'scan',
      label: t('command.scan'),
      group: t('command.actions'),
      icon: <Zap />,
      action: () => {
        void navigate({ to: '/' })
        startScan(settings.get().itemIds).catch(() => {})
      },
    },
    { id: 'clear', label: t('command.clear'), group: t('command.actions'), icon: <Trash2 />, action: () => settings.update({ itemIds: [] }) },
    {
      id: 'language',
      label: t('command.language'),
      group: t('command.actions'),
      icon: <Languages />,
      keywords: ['langue', 'language', 'français', 'english'],
      children: [
        { id: 'lang-fr', label: 'Français', action: () => settings.update({ language: 'fr' }) },
        { id: 'lang-en', label: 'English', action: () => settings.update({ language: 'en' }) },
      ],
    },
  ]

  return <CommandMenu
      actions={actions}
      placeholder={t('command.placeholder')}
      homeLabel={t('command.home')}
      emptyLabel={t('command.empty')}
    />
}
