import { isTauri } from '@tauri-apps/api/core'
import { openUrl as tauriOpenUrl } from '@tauri-apps/plugin-opener'

/** Ouvre un lien dans le navigateur du système (dans Tauri, un `<a target="_blank">` ne fait rien). */
export function openUrl(url: string) {
  if (isTauri()) void tauriOpenUrl(url)
  else window.open(url, '_blank', 'noopener')
}
