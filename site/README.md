# Site de Looty

Le site public du projet : page de présentation (français et anglais), **téléchargements** des applications,
métadonnées de **mise à jour** que lisent les applications, et **dépôt apt** signé pour Debian et Ubuntu.
Il est indépendant de GitHub : le dépôt du code peut rester privé, les fichiers publiés sont déposés ici par
l'intégration continue.

Deux morceaux, tous deux très légers :

| Dossier | Rôle |
|---|---|
| `web/` | Pages statiques, [Astro](https://astro.build) (aucune bibliothèque dans le navigateur ; ~40 Ko au total). |
| `server/` | Serveur Rust (axum), un seul fil, ~5 Mo de mémoire. Sert les pages et les fichiers, fabrique `/latest.json` et les liens stables, reçoit les dépôts de l'intégration continue. |

## Adresses

| URL | Contenu |
|---|---|
| `/`, `/fr/` | Présentation, téléchargements |
| `/latest.json` | Dernières versions de `android`, `linux` et `windows` : version, date, titre, notes, somme SHA-256, taille, adresse du fichier. Lu par l'application Android. |
| `/download/android`, `/download/linux`, `/download/windows` | Liens stables : redirigent vers le dernier fichier |
| `/releases/<plateforme>/…` | Les fichiers (APK, .deb, installateur Windows), avec reprise de téléchargement |
| `/apt/…` | Dépôt apt signé (`stable main`) |
| `PUT/DELETE /_upload/<chemin>`, `GET /_ls/<dossier>` | Dépôt de fichiers par l'intégration continue, avec le jeton (`Authorization: Bearer …`). Seuls les dossiers `releases/` et `apt/` sont acceptés. Désactivé si `UPLOAD_TOKEN` n'est pas défini. |

## Installer le site

1. Copier `docker-compose.example.yml` en `docker-compose.yml`, créer un `.env` :
   ```
   LOOTY_SITE_URL=https://looty.example.org
   LOOTY_SITE_UPLOAD_TOKEN=<au moins 40 caractères aléatoires : openssl rand -hex 32>
   ```
2. `docker compose up -d`, puis un proxy HTTPS devant `127.0.0.1:8080`. Avec Caddy :
   ```
   looty.example.org {
       reverse_proxy 127.0.0.1:8080
   }
   ```
3. Dans le dépôt GitHub : variable **`SITE_URL`** (la même adresse) et secret **`SITE_UPLOAD_TOKEN`** (le même jeton).
4. Dans `backend/releases.json`, champ **`"site"`** : la même adresse. Les serveurs Looty la donnent à leurs
   applications (`/api/releases`), qui savent ainsi où chercher les mises à jour. Un serveur peut aussi la
   fixer avec la variable `LOOTY_SITE_URL`.

Chaque version publiée par l'intégration continue (Android signé, `.deb`, installateur Windows) est alors
déposée ici ; les anciens fichiers sont retirés. Sans `SITE_URL`/`SITE_UPLOAD_TOKEN`, ces étapes sont ignorées.

**Dépôt apt** : une seule fois, `desktop/scripts/apt-key.sh` crée la clé de signature (clé publique à ajouter au
dépôt git, clé privée dans le secret `APT_GPG_PRIVATE_KEY`). Voir `desktop/README.md`.

## Développer

```sh
cd server && cargo test                         # tests du serveur
cd web && npm install && npm run dev            # pages avec rechargement
cd web && npm run build                         # → web/dist
WEB_DIR=web/dist DATA_DIR=/tmp/site UPLOAD_TOKEN=secret PORT=8080 server/target/debug/looty-site
```

Variables du serveur : `PORT` (8080), `WEB_DIR`, `DATA_DIR`, `UPLOAD_TOKEN`, `SITE_URL` (adresse publique ; sinon
déduite de l'en-tête `Host` / `X-Forwarded-*`).
