import { useTranslation } from 'react-i18next'

import { isCommandError } from './api'

/** Message traduit pour une erreur renvoyée par le moteur. */
export function useErrorMessage() {
  const { t } = useTranslation()
  return (error: unknown) =>
    isCommandError(error) ? t(`errors.${error.code}`, { defaultValue: error.message }) : t('errors.unknown')
}
