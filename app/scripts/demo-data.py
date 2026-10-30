"""Génère src/lib/demo-data.json (données du mode démo) à partir de vraies données.

Usage, depuis la racine du dépôt, après un scan réel fait avec la CLI :
    wowscan scan <ids…> --json > /tmp/scan.json
    python3 app/scripts/demo-data.py <dossier de cache> /tmp/scan.json

Seuls le français et l'anglais sont gardés, pour un fichier léger.
"""
import collections
import gzip
import json
import sys

DEMO_SETS = {4385, 4386, 4387, 4388, 5632, 5633, 5637, 5641, 5645, 171, 172, 173, 175, 176, 177}
LOCALES = ("fr_FR", "en_GB")

cache, scan_path = sys.argv[1], sys.argv[2]
read = lambda name: json.load(gzip.open(f"{cache}/{name}"))
slim = lambda text: {k: v for k, v in (text or {}).items() if k in LOCALES}

data = read("gamedata.json.gz")
infos = read("items.json.gz")
scan = json.load(open(scan_path))

def drops(item):
    out = []
    for encounter_id in data["drops"].get(item, []):
        encounter = data["encounters"].get(str(encounter_id))
        if encounter:
            out.append({
                "encounterId": encounter_id, "encounter": slim(encounter["name"]),
                "instanceId": encounter["instance"], "instance": slim(data["instances"].get(str(encounter["instance"]))),
            })
    return out

sources = collections.defaultdict(list)
for item, triples in data["item_appearances"].items():
    for modifier, appearance, kind in triples:
        sources[appearance].append({"itemId": int(item), "modifier": modifier, "appearanceId": appearance, "kind": kind, "drops": drops(item)})

sets, used_labels = [], set()
for s in data["sets"]:
    if s["id"] not in DEMO_SETS:
        continue
    pieces = {}
    for p in s["pieces"]:
        piece = pieces.setdefault(p["appearanceId"], {
            "appearanceId": p["appearanceId"], "slot": data["slots"].get(str(p["itemId"])),
            "setSources": [], "allSources": sources[p["appearanceId"]],
        })
        piece["setSources"].append(p)
    sets.append({
        "id": s["id"], "name": slim(s["name"]), "label": s["label"], "classMask": s["classMask"],
        "expansion": s["expansion"], "appearances": len(pieces), "pieces": list(pieces.values()),
    })
    if s["label"]:
        used_labels.add(s["label"])

# Les 12 royaumes les moins chers par version suffisent à la démo.
for item in scan["items"]:
    for variant in item["variants"]:
        used_labels.update(variant["labels"])
        variant["realms"] = variant["realms"][:12]

wanted = {i["itemId"] for i in scan["items"]} | {src["itemId"] for s in sets for p in s["pieces"] for src in p["setSources"]}
out = {
    "scannedAt": scan["scannedAt"],
    "realms": [{"id": r["id"], "realms": [{**x, "name": slim(x["name"])} for x in r["realms"]]} for r in scan["realms"]],
    "items": scan["items"],
    "infos": {
        str(i): {**info, **{k: slim(info[k]) for k in ("name", "itemClass", "itemSubclass", "inventoryType")}}
        for i, info in infos.items() if int(i) in wanted
    },
    "labels": {str(l): slim(data["labels"].get(str(l))) for l in sorted(used_labels) if str(l) in data["labels"]},
    "sets": sets,
}
json.dump(out, open("app/src/lib/demo-data.json", "w"), ensure_ascii=False, separators=(",", ":"))
print("demo-data.json :", len(out["items"]), "objets,", len(sets), "ensembles,", len(out["infos"]), "fiches")
