import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'

import { settings } from '@/lib/settings'

import { en } from './en'
import { fr } from './fr'

export type Language = 'fr' | 'en'

/** Locale de l'API Blizzard pour les noms d'objets et de royaumes. */
export const apiLocale = (language: string) => (language === 'en' ? 'en_GB' : 'fr_FR')

void i18n.use(initReactI18next).init({
  resources: { fr: { translation: fr }, en: { translation: en } },
  lng: settings.get().language,
  fallbackLng: 'fr',
  interpolation: { escapeValue: false },
})

settings.subscribe(() => {
  const { language } = settings.get()
  if (i18n.language !== language) void i18n.changeLanguage(language)
  document.documentElement.lang = language
})
document.documentElement.lang = settings.get().language

export default i18n
