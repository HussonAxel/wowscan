# wowscan

Application desktop (Windows) pour **World of Warcraft Retail, région Europe** : cherche un ou plusieurs objets sur l'hôtel des ventes de **tous les royaumes européens** et calcule la liste d'achat la moins chère. Pensée d'abord pour la transmogrification.

Interface en français et en anglais.

## Ce que fait la version actuelle

- **Scan à la demande** : colle des identifiants d'objets, des liens Wowhead ou des liens en jeu. Chercher 1 ou 200 objets prend le même temps : le coût vient du téléchargement des enchères des ~90 groupes de royaumes, pas du nombre d'objets.
- **Cache horaire** : Blizzard publie un instantané par heure. Un nouveau scan dans l'heure ne retélécharge rien (requête `If-Modified-Since`, réponse `304`).
- **Objets empilables détectés** : herbes, minerais, gemmes… ont un marché unique pour toute la région ; l'app l'indique au lieu de comparer des royaumes.
- **Liste d'achat optimisée** : tu coches les royaumes où tu as déjà un personnage et tu fixes ce que « vaut » un nouveau royaume, en or. L'app choisit où acheter pour minimiser le total. Les achats se ramènent ensuite par la banque de bataillon.
- **Identifiants protégés** : ils sont rangés dans le Gestionnaire d'identification de Windows, jamais dans un fichier.

## Démarrer (Windows)

Prérequis : [Rust](https://rustup.rs) (avec les *Build Tools* MSVC proposés par l'installateur), [Node.js 24](https://nodejs.org) et pnpm (`npm install -g pnpm`). WebView2 est déjà présent sur Windows 10/11.

```powershell
git clone https://github.com/HussonAxel/wowscan.git
cd wowscan/app
pnpm install
pnpm tauri dev      # lance l'app en mode développement
pnpm tauri build    # produit l'installateur dans target/release/bundle/
```

Chaque push sur `main` produit aussi l'installateur Windows dans l'onglet **Actions** de GitHub (artefact `wowscan-windows`).

### Identifiants Blizzard (gratuits, 2 minutes)

1. Va sur <https://develop.battle.net/access/clients> et connecte-toi avec ton compte Battle.net.
2. **Create client** : nom libre (par ex. `wowscan`). Les champs d'URL (redirection, service) ne servent pas pour l'instant.
3. Copie le **Client ID** et le **Client secret** dans *Paramètres → API Blizzard* : l'app les vérifie auprès de Blizzard avant de les enregistrer.

## Travailler sur l'interface sans Windows

`pnpm dev` dans `app/` sert l'interface dans un navigateur avec des **données de démo** (objets fictifs, vrais noms de royaumes). Aucun identifiant n'est nécessaire.

## Ligne de commande

Le moteur s'utilise aussi sans interface, sur n'importe quel système :

```bash
export WOWSCAN_CLIENT_ID=... WOWSCAN_CLIENT_SECRET=...
cargo run -p wowscan-cli -- check
cargo run -p wowscan-cli -- realms
cargo run -p wowscan-cli -- scan 19019 https://www.wowhead.com/item=2589 --owned 1080,1301 --new-realm-cost 500
```

## Architecture

```
crates/core/     moteur Rust : client API Blizzard, enchères, cache, fiches d'objets, optimisation
crates/cli/      `wowscan` en ligne de commande
app/             interface React (TanStack Router/Query/Table, Tailwind, shadcn + uiarc, beui, kobra)
app/src-tauri/   application Tauri : pont entre l'interface et le moteur
```

| Sujet | Choix |
|---|---|
| Données | API officielle Blizzard (Game Data) uniquement. Aucune lecture de la mémoire du jeu. |
| Cache | Dossier de cache de l'app ; enchères réduites (objet, prix unitaire, quantité) et compressées. |
| Optimisation | Problème de localisation d'entrepôts : départ glouton + recherche locale (ajout, retrait, échange de royaumes). |
| TLS | Natif sous Windows (schannel), rustls ailleurs. |
| Composants UI | Installés avec la CLI shadcn via pnpm. Les variables d'uiarc sont limitées à la classe `.arc` pour ne pas écraser celles de shadcn. |

Tests : `cargo test` (moteur, dont un scan complet contre un faux serveur Blizzard) et `pnpm lint && pnpm build` dans `app/`.

## Feuille de route

- **Apparences** : chercher par *apparence* plutôt que par objet (plusieurs objets partagent le même visuel) à partir des tables du jeu (wago.tools), et masquer les apparences déjà collectionnées.
- **Add-on en jeu** : exporter la collection de transmogrification (API `C_TransmogCollection`, plus complète que l'API web) et afficher la liste d'achat à l'hôtel des ventes.
- **Collections du compte** via Battle.net (OAuth) : montures, mascottes, jouets, hauts faits, réputations, et ce qu'il reste à obtenir.

## Licence

MIT. World of Warcraft et Battle.net sont des marques de Blizzard Entertainment ; ce projet n'est ni affilié ni approuvé par Blizzard.
