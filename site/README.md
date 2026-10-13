# Site de Looty

Le site public du projet : page de présentation (français et anglais), **téléchargements** des applications,
métadonnées de **mise à jour** que lisent les applications, et **dépôt apt** signé pour Debian et Ubuntu.
**Facultatif** : le dépôt est public, GitHub suffit pour distribuer les applications (voir `RELEASING.md`). Ce site
sert à avoir une belle page de présentation, des adresses de téléchargement stables et votre propre dépôt apt, sans
dépendre de GitHub : les fichiers publiés y sont déposés par l'intégration continue.

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

## Installer le site (Traefik)

1. **Sur le serveur du site**, avec `site/docker-compose.prod.yml` et un `.env` (modèle : `site/.env.example`) :
   ```
   SITE_DOMAIN=looty.untypequicode.fr
   SITE_UPLOAD_TOKEN=<openssl rand -hex 32>
   ```
   Les étiquettes Traefik sont celles de `docker-compose.prod.yml` (routeur `Host(\`${SITE_DOMAIN}\`)`, port 8080) : si votre
   Traefik utilise un réseau externe, des points d'entrée ou un résolveur de certificats précis, ajoutez les mêmes
   étiquettes/réseaux que ceux de votre déploiement de Looty. Le DNS de `looty.untypequicode.fr` doit pointer vers ce serveur.
2. **Publier l'image** (une fois) : GitHub → Actions → **Site** → *Run workflow*, ou un tag `site-v0.1.0`. Rendre le paquet
   `looty-site` public dans les réglages des paquets. Puis `docker compose -f docker-compose.prod.yml up -d`.
3. **Dans le dépôt GitHub** (Settings → Secrets and variables → Actions) :
   - onglet **Variables** → `SITE_URL` = `https://looty.untypequicode.fr`
   - onglet **Secrets** → `SITE_UPLOAD_TOKEN` = le même jeton que sur le serveur
   (ou : `gh variable set SITE_URL --body https://looty.untypequicode.fr` et `gh secret set SITE_UPLOAD_TOKEN`).
4. L'adresse est aussi dans `backend/releases.json` (champ **`"site"`**) : les serveurs Looty la donnent à leurs applications
   (`/api/releases`), qui savent ainsi où chercher les mises à jour ; un serveur peut la changer avec `LOOTY_SITE_URL`.
   Si le site est injoignable ou vide, l'application Android se rabat sur la publication GitHub.

Chaque version publiée par le workflow **Release** (Android signé, `.deb`, installateur Windows, dépôt apt) est déposée ici ;
les anciens fichiers sont retirés. Sans `SITE_URL`/`SITE_UPLOAD_TOKEN`, ces étapes sont ignorées.
Un gros fichier déposé à travers Traefik doit tenir dans son délai de lecture (60 s par défaut avec Traefik 3) : en cas
d'échec, augmenter `respondingTimeouts.readTimeout` du point d'entrée.

**Dépôt apt** : `desktop/scripts/apt-key.sh` crée la clé de signature (clé publique à commiter, clé privée dans le secret
`APT_GPG_PRIVATE_KEY`). Voir `desktop/README.md`.

## Développer

```sh
cd server && cargo test                         # tests du serveur
cd web && npm install && npm run dev            # pages avec rechargement
cd web && npm run build                         # → web/dist
WEB_DIR=web/dist DATA_DIR=/tmp/site UPLOAD_TOKEN=secret PORT=8080 server/target/debug/looty-site
```

Variables du serveur : `PORT` (8080), `WEB_DIR`, `DATA_DIR`, `UPLOAD_TOKEN`, `SITE_URL` (adresse publique ; sinon
déduite de l'en-tête `Host` / `X-Forwarded-*`).
