// Données de démo, pour travailler l'interface dans un navigateur sans Tauri ni identifiants.
// Les objets sont fictifs ; les noms de royaumes sont de vrais royaumes européens.
import type { Api } from './api'
import type { ConnectedRealm, ItemInfo, ItemResult, ScanEvent } from './types'

const realmGroups: string[][] = [
  ['Hyjal'],
  ['Archimonde'],
  ['Ysondre'],
  ['Dalaran', 'Marécage de Zangar'],
  ['Elune', 'Varimathras'],
  ['Kirin Tor', 'Sinstralis', 'Conseil des Ombres'],
  ['Confrérie du Thorium', 'Les Clairvoyants', 'Les Sentinelles'],
  ['Ragnaros'],
  ['Kazzak'],
  ['Silvermoon'],
  ['Draenor'],
  ['Tarren Mill'],
  ['Twisted Nether'],
  ['Stormscale'],
  ['Argent Dawn'],
]

const realms: ConnectedRealm[] = realmGroups.map((names, i) => ({
  id: 1000 + i,
  realms: names.map((name, j) => ({
    id: 1000 + i * 10 + j,
    name: { fr_FR: name, en_GB: name },
    slug: name.toLowerCase().replace(/\s+/g, '-'),
  })),
}))

const icon = (name: string) => `https://render.worldofwarcraft.com/eu/icons/56/${name}.jpg`
const items: ItemInfo[] = [
  { id: 900001, name: { fr_FR: 'Heaume du gardien solaire', en_GB: 'Sunwarden Helm' }, quality: 'EPIC', icon: icon('inv_helmet_03'), itemClass: { fr_FR: 'Armure', en_GB: 'Armor' }, itemSubclass: { fr_FR: 'Plaques', en_GB: 'Plate' }, inventoryType: { fr_FR: 'Tête', en_GB: 'Head' } },
  { id: 900002, name: { fr_FR: 'Cape du vent de minuit', en_GB: 'Midnight Gale Cloak' }, quality: 'RARE', icon: icon('inv_misc_cape_11'), itemClass: { fr_FR: 'Armure', en_GB: 'Armor' }, itemSubclass: { fr_FR: 'Tissu', en_GB: 'Cloth' }, inventoryType: { fr_FR: 'Dos', en_GB: 'Back' } },
  { id: 900003, name: { fr_FR: 'Lame du crépuscule ardent', en_GB: 'Blade of the Burning Dusk' }, quality: 'EPIC', icon: icon('inv_sword_39'), itemClass: { fr_FR: 'Arme', en_GB: 'Weapon' }, itemSubclass: { fr_FR: 'Épée à une main', en_GB: 'One-Handed Sword' }, inventoryType: { fr_FR: 'Une main', en_GB: 'One-Hand' } },
  { id: 900004, name: { fr_FR: 'Bâton du sage errant', en_GB: 'Staff of the Wandering Sage' }, quality: 'UNCOMMON', icon: icon('inv_staff_13'), itemClass: { fr_FR: 'Arme', en_GB: 'Weapon' }, itemSubclass: { fr_FR: 'Bâton', en_GB: 'Staff' }, inventoryType: { fr_FR: 'Deux mains', en_GB: 'Two-Hand' } },
  { id: 900005, name: { fr_FR: 'Fleur de paix', en_GB: 'Peacebloom' }, quality: 'COMMON', icon: icon('inv_misc_flower_02'), itemClass: { fr_FR: 'Artisanat', en_GB: 'Tradeskill' }, itemSubclass: { fr_FR: 'Herbe', en_GB: 'Herb' }, inventoryType: { fr_FR: 'Non équipable', en_GB: 'Non-equippable' } },
]
const itemsById = new Map(items.map((i) => [i.id, i]))

/** Générateur pseudo-aléatoire déterministe : les mêmes prix à chaque scan. */
function random(seed: number) {
  let s = seed
  return () => {
    s = (s * 1664525 + 1013904223) % 2 ** 32
    return s / 2 ** 32
  }
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

function demoParse(input: string) {
  const cleaned = input.replace(/\[[^\]]*\]/g, ' ')
  const itemIds: number[] = []
  const unrecognized: string[] = []
  for (const token of cleaned.split(/[\s,;]+/)) {
    if (!token || token === '|h|r') continue
    const ids = /^\d+$/.test(token)
      ? [Number(token)]
      : [...token.matchAll(/item[=:](\d+)/g)].map((m) => Number(m[1]))
    if (!ids.length) unrecognized.push(token)
    for (const id of ids) if (id > 0 && !itemIds.includes(id)) itemIds.push(id)
  }
  return { itemIds, unrecognized }
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
  connectedRealms: async () => realms,
  itemInfos: async (ids) =>
    Object.fromEntries(ids.flatMap((id) => (itemsById.has(id) ? [[id, itemsById.get(id)!]] : []))),
  scan: async (ids, onEvent) => {
    const total = realms.length + 1
    onEvent({ type: 'started', total })
    for (let done = 1; done <= total; done++) {
      await sleep(60 + (done % 4) * 40)
      const realmId = done === 1 ? null : realms[done - 2].id
      const event: ScanEvent =
        done === 9
          ? { type: 'sourceFailed', realmId, error: "l'API Blizzard a répondu 503", done, total }
          : { type: 'sourceDone', realmId, downloaded: done % 3 !== 0, done, total }
      onEvent(event)
    }
    const results: ItemResult[] = ids.map((itemId) => {
      const rand = random(itemId)
      if (itemId === 900005) {
        return { itemId, commodity: { min: 1_850, auctions: 412, quantity: 18_920 }, realms: [] }
      }
      if (!itemsById.has(itemId)) return { itemId, commodity: null, realms: [] }
      const base = 2_000_000 + Math.floor(rand() * 40_000_000)
      const priced = realms
        .filter((r) => r.id !== 1007 && rand() > 0.35)
        .map((r) => ({
          connectedRealmId: r.id,
          stats: { min: Math.floor(base * (0.4 + rand() * 1.6)), auctions: 1 + Math.floor(rand() * 4), quantity: 1 },
        }))
        .sort((a, b) => a.stats.min - b.stats.min)
      return { itemId, commodity: null, realms: priced }
    })
    return {
      realms,
      items: results,
      failures: [{ realmId: realms[7].id, error: "l'API Blizzard a répondu 503" }],
      scannedAt: Math.floor(Date.now() / 1000),
    }
  },
  // Démo seulement : le moins cher par objet. Le vrai calcul (avec coût d'un nouveau royaume) est en Rust.
  plan: async ({ wants, ownedRealms, newRealmCost }) => {
    const purchases = wants.flatMap((w) => {
      const best = [...w.candidates].sort((a, b) => a.price - b.price)[0]
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
