import { type QueryClient, type UseQueryResult, useQueries } from '@tanstack/react-query'
import { useEffect } from 'react'

import { api } from '@/lib/api'
import type { ItemInfo } from '@/lib/types'

/** Une icône s'affiche dès que son lot arrive : des lots courts, pour ne pas attendre le plus lent d'une longue liste. */
const BATCH_SIZE = 8
const URGENT_BATCHES = 2
/** Le préchargement passe après tout le reste, un lot à la fois et avec une pause. */
const BACKGROUND_PAUSE_MS = 300

interface Waiter {
  resolve: (item: ItemInfo) => void
  reject: (error: unknown) => void
}

const waiters = new Map<number, Waiter[]>()
let urgent: number[] = []
let background: number[] = []
let active = 0
let paused = false
let scheduled = false

function schedule() {
  if (scheduled) return
  scheduled = true
  // Attend la fin du rendu : les demandes d'un même écran partent ensemble.
  setTimeout(() => {
    scheduled = false
    pump()
  }, 0)
}

function pump() {
  while (active < URGENT_BATCHES && urgent.length > 0) run(urgent.splice(0, BATCH_SIZE), false)
  if (active === 0 && !paused && background.length > 0) run(background.splice(0, BATCH_SIZE), true)
}

function run(ids: number[], isBackground: boolean) {
  active += 1
  const settle = (one: (id: number, waiter: Waiter) => void) => {
    for (const id of ids) {
      for (const waiter of waiters.get(id) ?? []) one(id, waiter)
      waiters.delete(id)
    }
  }
  api
    .itemInfos(ids)
    .then((items) =>
      settle((id, waiter) => {
        const item = items[id]
        // Absente (échec passager ou objet inconnu) : la requête échoue et sera retentée au prochain affichage.
        if (item) waiter.resolve(item)
        else waiter.reject(new Error(`item ${id} unavailable`))
      }),
    )
    .catch((error: unknown) => settle((_, waiter) => waiter.reject(error)))
    .finally(() => {
      active -= 1
      if (isBackground) {
        paused = true
        setTimeout(() => {
          paused = false
          pump()
        }, BACKGROUND_PAUSE_MS)
      }
      pump()
    })
}

function loadItem(id: number, isBackground: boolean) {
  return new Promise<ItemInfo>((resolve, reject) => {
    const known = waiters.get(id)
    if (known) known.push({ resolve, reject })
    else {
      waiters.set(id, [{ resolve, reject }])
      if (isBackground) background.push(id)
      else urgent.push(id)
    }
    schedule()
  })
}

const itemKey = (id: number) => ['item', id]

function combineItems(results: UseQueryResult<ItemInfo>[]) {
  return Object.fromEntries(results.flatMap((result) => (result.data ? [[result.data.id, result.data]] : []))) as Record<number, ItemInfo>
}

/** Fiches d'objets, gardées une par une : chacune apparaît dès qu'elle est prête et sert à tous les écrans. */
export function useItemInfos(ids: number[], enabled = true) {
  const key = ids.join(',')
  useEffect(() => {
    // Ce qui est à l'écran passe devant le préchargement, déjà lancé pour ces objets.
    if (!enabled) return
    const wanted = new Set(key ? key.split(',').map(Number) : [])
    const promoted = background.filter((id) => wanted.has(id))
    if (promoted.length === 0) return
    background = background.filter((id) => !wanted.has(id))
    urgent = [...urgent, ...promoted]
    schedule()
  }, [key, enabled])
  return useQueries({
    queries: ids.map((id) => ({ queryKey: itemKey(id), queryFn: () => loadItem(id, false), enabled })),
    combine: combineItems,
  })
}

/** Charge des fiches en tâche de fond, à débit limité ; les échecs seront retentés à l'affichage. */
export function prefetchItems(queryClient: QueryClient, ids: Iterable<number>) {
  for (const id of ids) {
    void queryClient.prefetchQuery({ queryKey: itemKey(id), queryFn: () => loadItem(id, true) })
  }
}
