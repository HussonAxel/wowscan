import { useQuery } from '@tanstack/react-query'
import { convertFileSrc, isTauri } from '@tauri-apps/api/core'
import { useState } from 'react'

import { api } from './api'
import { useSettings } from './settings'

/** Version de l'icône dans un pack installé : le dernier segment de l'URL Blizzard est un nom ou un numéro de fichier. */
function packUrl(url: string, gameDir: string) {
  const file = /\/([^/]+)\.jpg$/.exec(url)?.[1]
  return file ? `${convertFileSrc(file, 'wowicon')}?dir=${encodeURIComponent(gameDir)}` : null
}

/** Choisit où charger une icône Blizzard : le pack installé s'il la contient, sinon le serveur d'images. */
export function useGameIcon() {
  const { gameDir } = useSettings()
  const status = useQuery({
    queryKey: ['icon-game-dir', gameDir],
    queryFn: () => api.collectionStatus(gameDir || null),
    select: (data) => data.gameDir,
    staleTime: Infinity,
    enabled: isTauri(),
  })
  const [broken, setBroken] = useState<ReadonlySet<string>>(new Set())
  // On attend de connaître le dossier du jeu pour ne pas charger l'icône deux fois.
  const ready = !isTauri() || !status.isPending
  return {
    ready,
    pick(url: string | null | undefined) {
      if (!url || !ready) return undefined
      const pack = status.data ? packUrl(url, status.data) : null
      return [pack, url].find((source): source is string => source !== null && !broken.has(source))
    },
    fail: (src: string) => setBroken((current) => new Set(current).add(src)),
  }
}
