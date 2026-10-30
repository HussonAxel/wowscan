// Mode démo (navigateur, sans Tauri ni identifiants). Les données viennent d'un vrai scan
// (voir scripts/demo-data.py) ; ce module n'est chargé que dans ce mode.
import type { Api } from './api'
import demo from './demo-data.json'
import type {
  CollectionStatus,
  ConnectedRealm,
  ItemInfo,
  ItemRef,
  ItemResult,
  LocalizedString,
  ScanEvent,
  SetCollection,
  SetCategory,
  SetDetails,
  SetDifficulty,
  SetRaid,
  Variant,
} from './types'

const realms = demo.realms as ConnectedRealm[]
const infos = demo.infos as unknown as Record<string, ItemInfo>
const results = demo.items as ItemResult[]
const labels = demo.labels as unknown as Record<number, LocalizedString>
/** Les données de démo n'ont pas de catégorie : on la déduit du libellé, comme le moteur. */
const demoCategory = (label: number | null): SetCategory => {
  const text = label === null ? '' : (labels[label]?.en_GB ?? '')
  if (/^(Gladiator|Elite|Combatant|Aspirant|Honor|PvP)/i.test(text)) return 'pvp'
  if (/^(Raid Finder|Normal|Heroic|Mythic|\d+ Player)/.test(text)) return 'raid'
  if (/^(Dungeon|Timewarped)/.test(text)) return 'dungeon'
  return 'other'
}
const demoDifficulty = (label: number | null): SetDifficulty => {
  const text = label === null ? '' : (labels[label]?.en_GB ?? '')
  return text.startsWith('Raid Finder') ? 'raidFinder' : text.includes('Heroic') ? 'heroic' : text.startsWith('Mythic') ? 'mythic' : 'normal'
}
const demoRaid = (set: Omit<SetDetails, 'category' | 'raid'>): SetRaid | null => {
  const drop = set.pieces.flatMap((p) => p.allSources.flatMap((s) => s.drops))[0]
  if (demoCategory(set.label) !== 'raid' || !drop) return null
  return { instanceId: drop.instanceId, name: drop.instance, difficulty: demoDifficulty(set.label) }
}
const sets = (demo.sets as Omit<SetDetails, 'category' | 'raid'>[]).map((s) => ({
  ...s,
  category: demoCategory(s.label),
  raid: demoRaid(s),
}))

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))
const fold = (text: string) =>
  text.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[’‘]/g, "'")

/** Bonus connus de la démo (extraits réels des tables du jeu, ceinture 124182). */
const demoBonus: Record<number, { modifier: number; labels: number[] }> = {
  9080: { modifier: 1, labels: [2015] },
  9081: { modifier: 3, labels: [13145] },
  9082: { modifier: 4, labels: [1641] },
  1799: { modifier: 3, labels: [13145] },
}
const demoAppearances: Record<number, Record<number, number>> = { 124182: { 0: 25447, 1: 25419, 3: 25526 } }

function demoVariant({ itemId, bonusLists }: ItemRef): Variant {
  const bonus = bonusLists.map((b) => demoBonus[b]).find(Boolean)
  const modifier = bonus?.modifier ?? 0
  const appearanceId =
    demoAppearances[itemId]?.[modifier] ??
    results.find((i) => i.itemId === itemId)?.variants.find((v) => v.modifier === modifier)?.appearanceId ??
    null
  return { modifier, appearanceId, labels: bonus?.labels ?? [] }
}

/** Version simplifiée de `crates/core/src/refs.rs`, suffisante pour la démo. */
function demoParse(input: string) {
  const items: ItemRef[] = []
  const push = (itemId: number, bonusLists: number[]) => {
    if (itemId > 0 && !items.some((i) => i.itemId === itemId && i.bonusLists.join() === bonusLists.join())) {
      items.push({ itemId, bonusLists })
    }
  }
  let rest = input.replace(/\|Hitem:([^|]*)\|h\[[^\]]*\]\|h/g, (_, data: string) => {
    const fields = data.split(':')
    const count = Number(fields[12]) || 0
    push(Number(fields[0]), fields.slice(13, 13 + count).map(Number).filter(Boolean))
    return ' '
  })
  rest = rest.replace(/\|c[0-9a-fA-F]{8}|\|cn[A-Za-z0-9_]+:|\|r/g, '')
  const names = [...rest.matchAll(/\[([^[\]]+)\]/g)]
    .map((m) => m[1].trim())
    .filter((n) => /\p{L}/u.test(n) && !/^(\d+\.\s|\d{1,2}:\d{2})/.test(n))
  rest = rest.replace(/\[[^[\]]*\]/g, ' ')
  const tokens = rest.split(/[\s,;]+/).filter(Boolean)
  const isList = tokens.every((t) => /^\d+$/.test(t) || /item[=:]\d+/.test(t))
  for (const token of tokens) {
    const id = /item[=:](\d+)/.exec(token)?.[1]
    if (id) push(Number(id), (/[?&]bonus=([\d:]+)/.exec(token)?.[1] ?? '').split(':').map(Number).filter(Boolean))
    else if (isList && /^\d+$/.test(token)) push(Number(token), [])
  }
  return { items, names: [...new Set(names)], battlePets: [], appearances: [], unrecognized: [] }
}

type Want = Parameters<Api['plan']>[0]['wants'][number]

/**
 * Même méthode que `crates/core/src/optimize.rs` (point de départ le moins cher par ligne,
 * puis ajout, retrait ou échange de royaumes tant que le total baisse), en version courte.
 */
function demoPlanRealms(wants: Want[], owned: Set<number>, newRealmCost: number): Set<number> {
  const lines = wants.filter((w) => w.candidates.length)
  const cost = (open: Set<number>) => {
    let total = 0
    for (const w of lines) {
      const prices = w.candidates.filter((c) => open.has(c.connectedRealmId)).map((c) => c.price)
      if (!prices.length) return Infinity
      total += Math.min(...prices)
    }
    return total + [...open].filter((r) => !owned.has(r)).length * newRealmCost
  }
  const all = [...new Set(lines.flatMap((w) => w.candidates.map((c) => c.connectedRealmId)))]
  let open = new Set(lines.map((w) => [...w.candidates].sort((a, b) => a.price - b.price)[0].connectedRealmId))
  let best = cost(open)
  for (;;) {
    let move: Set<number> | null = null
    const consider = (set: Set<number>) => {
      const c = cost(set)
      if (c < best) [best, move] = [c, set]
    }
    for (const r of open) {
      const without = new Set([...open].filter((x) => x !== r))
      consider(without)
      for (const add of all.filter((x) => !open.has(x))) consider(new Set([...without, add]))
    }
    for (const add of all.filter((x) => !open.has(x))) consider(new Set([...open, add]))
    if (!move) return open
    open = move
  }
}

/** Collection de démo : une apparence sur trois manque, sauf pour le premier ensemble, complet. */
const demoCollected = new Set(
  sets.flatMap((s, index) => s.pieces.map((p) => p.appearanceId).filter((a) => index === 0 || a % 3 !== 0)),
)
let demoAddon: CollectionStatus['addon'] = 'outdated'

function demoStatus(): CollectionStatus {
  return {
    gameDir: 'C:\\Program Files (x86)\\World of Warcraft\\_retail_',
    addon: demoAddon,
    scannedAt: demo.scannedAt,
    appearances: demoCollected.size,
    characters: [
      { name: 'Ayije', realm: 'Elune', classId: 3 },
      { name: 'Funel', realm: 'Kirin Tor', classId: 9 },
    ],
  }
}

/** Même calcul que `set_progress` (crates/core/src/collection.rs) : par emplacement, robe = torse. */
function demoProgress(set: SetDetails): SetCollection {
  const slots = new Map<string, number[]>()
  for (const piece of set.pieces) {
    const key = piece.slot ? String(piece.slot === 20 ? 5 : piece.slot) : `a${piece.appearanceId}`
    slots.set(key, [...(slots.get(key) ?? []), piece.appearanceId])
  }
  const groups = [...slots.values()]
  const missing = groups.filter((g) => !g.some((a) => demoCollected.has(a)))
  const { pieces: _pieces, ...summary } = set
  return { ...summary, collected: groups.length - missing.length, total: groups.length, missing: missing.flat() }
}

let demoCredentials = true

export const mockApi: Api = {
  hasCredentials: async () => demoCredentials,
  saveCredentials: async () => {
    await sleep(600)
    demoCredentials = true
  },
  clearCredentials: async () => {
    demoCredentials = false
  },
  parseItems: async (input) => demoParse(input),
  findItemsByName: async (names) =>
    Object.fromEntries(
      names.map((name) => [
        name,
        Object.values(infos)
          .filter((i) => Object.values(i.name).some((n) => fold(n) === fold(name)))
          .map((i) => i.id),
      ]),
    ),
  resolveVariants: async (items) => items.map(demoVariant),
  variantLabels: async () => labels,
  searchSets: async (query) => {
    const words = fold(query).split(/\s+/).filter(Boolean)
    if (!words.length) return []
    return sets
      .filter((s) => Object.values(s.name).some((n) => words.every((w) => fold(n).includes(w))))
      .map(({ pieces: _pieces, ...summary }) => summary)
  },
  setDetails: async (id) => sets.find((s) => s.id === id) ?? null,
  appearanceItems: async (ids) =>
    Object.fromEntries(
      ids.map((id) => [
        id,
        [...new Set(sets.flatMap((s) => s.pieces).filter((p) => p.appearanceId === id).flatMap((p) => p.allSources.map((src) => src.itemId)))],
      ]),
    ),
  collectionStatus: async () => demoStatus(),
  installAddon: async () => {
    await sleep(400)
    demoAddon = 'installed'
    return demoStatus()
  },
  collectionSets: async () => {
    await sleep(200)
    return sets.map(demoProgress)
  },
  collectionMounts: async () => {
    await sleep(200)
    const name = (text: string) => ({ en_GB: text, fr_FR: text })
    return {
      exported: true,
      mounts: [
        { id: 1818, spellId: 1, itemId: null, icon: null, name: name('Raid mount'), source: name('Drop: Boss'), category: 'raid' as const, expansion: 9, collected: true },
        { id: 1819, spellId: 2, itemId: null, icon: null, name: name('Rare mount'), source: name('Drop: Rare'), category: 'elite' as const, expansion: 9, collected: false },
        { id: 395, spellId: 3, itemId: null, icon: null, name: name('Faction mount'), source: name('Faction: Ramkahen'), category: 'reputation' as const, expansion: 3, collected: false },
      ],
    }
  },
  mountTooltip: async () => {
    const name = (text: string) => ({ en_GB: text, fr_FR: text })
    return {
      quality: 'EPIC',
      level: name('Item Level 10'),
      binding: name('Binds when picked up'),
      kind: name('Mount'),
      spells: [name('Teaches you how to summon this mount.')],
      requirements: [name('Requires Apprentice Riding')],
      description: name('A demo mount.'),
    }
  },
  connectedRealms: async () => realms,
  itemInfos: async (ids) => Object.fromEntries(ids.flatMap((id) => (infos[id] ? [[id, infos[id]]] : []))),
  scan: async (ids, onEvent) => {
    const total = realms.length + 1
    onEvent({ type: 'started', total })
    for (let done = 1; done <= total; done += 3) {
      await sleep(40)
      const event: ScanEvent = { type: 'sourceDone', realmId: realms[done - 1]?.id ?? null, downloaded: done % 2 === 0, done, total }
      onEvent(event)
    }
    onEvent({ type: 'sourceDone', realmId: null, downloaded: false, done: total, total })
    return {
      realms,
      items: ids.map((itemId) => results.find((i) => i.itemId === itemId) ?? { itemId, variants: [] }),
      failures: [],
      scannedAt: demo.scannedAt,
    }
  },
  plan: async ({ wants, ownedRealms, newRealmCost }) => {
    const open = demoPlanRealms(wants, new Set(ownedRealms), newRealmCost)
    const purchases = wants.flatMap((w) => {
      const best = w.candidates.filter((c) => open.has(c.connectedRealmId)).sort((a, b) => a.price - b.price)[0]
      return best ? [{ itemId: w.itemId, connectedRealmId: best.connectedRealmId, price: best.price }] : []
    })
    const used = [...new Set(purchases.map((p) => p.connectedRealmId))].sort((a, b) => a - b)
    const newRealms = used.filter((r) => !ownedRealms.includes(r))
    const itemsCost = purchases.reduce((sum, p) => sum + p.price, 0)
    return {
      purchases,
      realms: used,
      newRealms,
      itemsCost,
      newRealmsCost: newRealms.length * newRealmCost,
      total: itemsCost + newRealms.length * newRealmCost,
      unavailable: wants.filter((w) => !w.candidates.length).map((w) => w.itemId),
    }
  },
}
