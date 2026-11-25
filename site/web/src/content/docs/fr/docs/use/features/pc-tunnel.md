---
title: "Utiliser mon PC comme serveur Jupyter ou R"
description: "Reliez votre propre ordinateur à un projet Looty par une seule connexion sortante pour y exécuter vos notebooks Jupyter, Python et R : prérequis, pas à pas, commandes, sécurité et dépannage."
---

Un **tunnel PC** permet à Looty d'utiliser **votre propre ordinateur** comme serveur Jupyter d'un projet, **tant que le tunnel est ouvert**. Un petit programme, `looty-bridge` (l'*agent*), tourne sur votre PC et ouvre **une seule connexion sortante** vers votre instance Looty. Vous n'ouvrez aucun port et ne changez rien à votre box ou à votre pare-feu : cela fonctionne derrière un NAT.

C'est pensé pour les besoins lourds que le navigateur ne couvre pas : beaucoup de mémoire, un GPU, vos propres données, tous les paquets de votre machine.

## WebR et Pyodide, ou votre PC ?

| | Dans le navigateur (Pyodide, WebR) | Votre PC (tunnel) |
| --- | --- | --- |
| Installation | Rien | Docker ou Nix (ou un binaire téléchargé) |
| Disponibilité | Toujours, même hors ligne une fois chargé | Seulement tant que l'agent tourne sur votre PC |
| Paquets | Ceux compilés pour WebAssembly | Tous les paquets de votre Jupyter |
| Puissance | Mémoire et processeur du navigateur | Mémoire, processeur, GPU de votre PC |
| Interrompre un calcul | R : redémarrage seulement | Vraie interruption |
| Qui exécute du code | Chaque personne, dans son navigateur | Toute personne qui peut modifier le projet, **sur votre PC** |

Commencez par les moteurs du navigateur (voir [Projets et notebooks](/fr/docs/use/features/projects-notebooks/)). Utilisez votre PC quand ils ne suffisent pas.

:::note
L'administrateur doit avoir activé les tunnels PC (désactivés par défaut). Si le bouton **Utiliser mon PC** n'apparaît pas, demandez-lui : [côté hébergement](/fr/docs/host/operate/tunnels/). La fonction n'est pas disponible dans la démo publique.
:::

## Prérequis

- Un projet que vous pouvez **modifier**, dans une instance Looty où les tunnels sont activés.
- Sur le PC, **l'un** de ces choix : Docker avec Docker Compose (recommandé, tout est inclus), Nix, ou le binaire `looty-bridge` avec un serveur Jupyter que vous lancez déjà.
- Le PC doit joindre votre instance en HTTPS (un proxy HTTP qui n'autorise que le TCP simple n'est pas encore pris en charge : l'agent se connecte directement).
- Le PC doit rester allumé, et l'agent lancé, pendant que vous travaillez.

## Pas à pas

1. Dans le notebook, ouvrez la fenêtre **Moteur**, choisissez **Serveur Jupyter**, puis **Utiliser mon PC**.
2. Choisissez le service (Jupyter avec Python, R ou d'autres noyaux) et l'adresse de Jupyter sur votre PC (`http://127.0.0.1:8888` par défaut). Lisez l'avertissement (voir Sécurité plus bas) et cochez la confirmation.
3. Looty crée le **jeton de tunnel** (`lty_tun_...`) et l'affiche **une seule fois**, avec la ligne à lancer sur votre PC, l'adresse de l'instance déjà remplie. Copiez-la. Si vous perdez le jeton, révoquez le tunnel et créez-en un autre.
4. Lancez la ligne sur votre PC (ci-dessous). La fenêtre montre le statut en direct : en attente de l'agent, puis connecté (nom de la machine, latence).
5. Les noyaux de votre Jupyter apparaissent comme ceux de n'importe quel serveur Jupyter : choisissez-en un (par exemple Python 3, ou `ir` pour R) et exécutez vos cellules.
6. Pour arrêter, faites Ctrl+C sur le PC, ou utilisez **Déconnecter et révoquer** dans Looty, qui coupe la connexion immédiatement. Tous vos tunnels sont listés, avec un bouton de révocation, dans **Paramètres, Tunnels PC**.

## Les commandes

Le jeton se donne par `LOOTY_TOKEN` (à préférer, il reste hors de l'historique du shell) ou `--token`.

### Docker Compose (Jupyter avec Python et R inclus)

Depuis une copie du dépôt, créez un fichier `.env` :

```sh
LOOTY_INSTANCE=https://looty.example.org
LOOTY_TOKEN=lty_tun_...
JUPYTER_TOKEN=une-longue-chaine-aleatoire
```

puis lancez l'agent et un Jupyter avec des noyaux Python et R (publié uniquement sur `127.0.0.1:8888`) :

```sh
docker compose -f docker-compose.bridge.yml --profile jupyter up
```

Dans Looty, donnez au projet le même `JUPYTER_TOKEN` (stocké chiffré, jamais réaffiché). Sans `--profile jupyter`, seul l'agent démarre, pour un Jupyter que vous lancez déjà : définissez `BRIDGE_SERVICES=jupyter=http://host.docker.internal:8888`.

### Nix

```sh
export LOOTY_TOKEN=lty_tun_...
nix run github:untypequicode/looty#bridge -- --instance https://looty.example.org --service jupyter=http://127.0.0.1:8888
```

### Binaire

Téléchargez `looty-bridge-<version>-<os>-<arch>` (Linux x86_64 et aarch64, macOS, Windows) depuis les versions marquées `bridge-v*` sur [GitHub](https://github.com/untypequicode/looty/releases), puis :

```sh
LOOTY_TOKEN=lty_tun_... ./looty-bridge --instance https://looty.example.org --service jupyter=http://127.0.0.1:8888
```

Pour lancer d'abord votre propre Jupyter (boucle locale seulement, avec un jeton) :

```sh
jupyter lab --no-browser --ServerApp.ip=127.0.0.1 --IdentityProvider.token=<jeton>
```

### Options

| Option | Environnement | Rôle |
| --- | --- | --- |
| `--instance URL` | `LOOTY_INSTANCE` | Adresse de l'instance (`https://...`). |
| `--token T` | `LOOTY_TOKEN` | Jeton de tunnel. |
| `--service NOM=URL` | `LOOTY_SERVICES` (séparés par des virgules) | Service local à exposer, répétable. `NOM:TYPE=URL` fixe le type (`jupyter`, `latex`, `http`). Adresses locales `http://` uniquement. |

L'agent se reconnecte seul après une coupure réseau (attente de 1 s, doublée jusqu'à 60 s). Il s'arrête définitivement quand le jeton est révoqué, expiré ou utilisé par un autre agent.

## Quels noyaux, et R

Le tunnel transporte ce que propose votre Jupyter. Avec le fichier Docker Compose, vous avez Python 3 et R (IRkernel, `ir`, avec les paquets R usuels de l'image `r-notebook`). Avec votre propre Jupyter, installez les noyaux voulus (`IRkernel` pour R, Julia, etc.). Les notebooks R sur un PC bénéficient de la vraie interruption, des graphiques SVG et de la complétion du noyau, et les [panneaux R](/fr/docs/use/features/projects-notebooks/) fonctionnent de la même façon.

LaTeX : le type de service `latex` est un service de compilation qui tourne sur votre PC (`looty-latex-service`, avec le TeX Live de la machine). Avec Docker Compose, démarrez le profil `latex` et indiquez-le au bridge : `BRIDGE_SERVICES=latex=http://texlive:8080 docker compose -f docker-compose.bridge.yml --profile latex up` (`jupyter=http://jupyter:8888,latex=http://texlive:8080` pour les deux). Sans Docker, lancez `looty-latex-service` et ajoutez `--service tex:latex=http://127.0.0.1:8080` au bridge. Ensuite, dans un projet LaTeX, utilisez **Mon PC** à côté du choix du moteur : *Compiler* envoie le projet à votre PC, sans exécution de commandes et avec une limite de temps, et ne compile nulle part ailleurs si le tunnel est coupé.

## Sécurité et vie privée

**Connecter un Jupyter à un projet permet à toute personne qui peut modifier ce projet d'exécuter du code sur votre PC**, avec les droits de l'utilisateur qui lance Jupyter, chacune dans son propre noyau. Looty vous demande de le confirmer à la création du tunnel.

- **Seuls les services déclarés sont joignables.** L'instance ne peut parler qu'aux services que votre agent déclare, à l'adresse que **vous** avez configurée sur le PC. Elle ne peut atteindre ni un autre port de votre PC ni quoi que ce soit de votre réseau, et ne peut rien faire quand l'agent est arrêté.
- **Aucun port entrant.** L'agent se connecte vers l'extérieur ; rien n'écoute sur votre PC pour le tunnel.
- **Propriétaire seulement.** Vous pouvez réserver un tunnel à vous seul : les autres membres du projet ne peuvent alors pas s'en servir. Modifiable à tout moment.
- **Jetons.** Le jeton de tunnel est lié à vous et à un espace, expire (30 jours par défaut), est stocké haché sur l'instance, montré une seule fois, et ne sert qu'à la connexion du tunnel : il ne peut pas appeler le reste de l'API de Looty. La révocation coupe la connexion aussitôt. Gardez-le secret comme un mot de passe.
- **Audit.** L'administrateur de l'instance voit quand les tunnels sont créés, rattachés, connectés, déconnectés et révoqués. Les jetons n'y figurent jamais.
- **Le navigateur ne voit jamais votre PC.** Seul le serveur Looty parle au tunnel, comme pour tout serveur Jupyter de projet. Le jeton de votre Jupyter est stocké chiffré et jamais affiché.
- **Votre PC est de confiance pour le projet.** Les sorties d'un agent volé ou falsifié pourraient passer pour de vrais résultats : révoquez un jeton que vous pensez fuité.

Bonnes pratiques : rattachez le tunnel à un projet que vous seul pouvez modifier (par exemple dans votre espace personnel) ou cochez *Propriétaire seulement* ; lancez Jupyter dans un conteneur ou avec un utilisateur sans privilèges ; liez-le à `127.0.0.1` ; gardez le jeton de Jupyter activé.

## Limites

- 16 requêtes ou connexions de noyau simultanées par tunnel (l'administrateur peut l'abaisser), et un plafond de débit facultatif.
- Requêtes jusqu'à 16 Mio, réponses jusqu'à 64 Mio, messages de noyau jusqu'à 8 Mio.
- Une connexion de noyau qui n'envoie rien pendant 30 minutes est coupée ; une requête HTTP sans données pendant 60 secondes est annulée.
- Un seul agent par jeton : en lancer un second avec le même jeton arrête le premier.
- Si le PC se déconnecte, les notebooks affichent un message clair et rien n'est redirigé en silence. Relancez l'agent, ou repassez le notebook sur le moteur du navigateur en attendant. Le travail déjà enregistré dans le notebook n'est pas perdu.

## Dépannage

| Ce que vous voyez | Que faire |
| --- | --- |
| Pas de bouton **Utiliser mon PC** | Les tunnels sont désactivés sur cette instance (demandez à l'administrateur) ou vous êtes dans la démo. |
| L'agent s'arrête aussitôt, « revoked » ou « expired » | Le jeton n'est plus valide. Créez un nouveau tunnel dans Looty. |
| L'agent réessaie sans fin, l'instance répond 404 | Les tunnels PC sont désactivés dans l'Administration. |
| L'agent dit « replaced » et s'arrête | Un autre agent a démarré avec le même jeton. N'en gardez qu'un. |
| Le statut reste « en attente de l'agent » | Vérifiez l'adresse de l'instance (`https://`, sans chemin), le jeton, et que le PC joint l'instance. Un proxy qui coupe les WebSockets inactifs après un court délai provoque aussi ce cas : prévenez votre administrateur ([hébergement](/fr/docs/host/operate/tunnels/)). |
| Connecté, mais aucun noyau listé | L'adresse de Jupyter est fausse ou Jupyter ne tourne pas. Avec Docker, utilisez `jupyter=http://jupyter:8888` (défaut de Compose) ou `host.docker.internal` pour un Jupyter sur l'hôte. |
| Jupyter refuse (403 ou 401) | Le jeton Jupyter donné dans Looty ne correspond pas à celui de votre Jupyter (`JUPYTER_TOKEN`). |
| « Tunnel occupé » | Trop de requêtes simultanées sur le tunnel ; patientez ou demandez à l'administrateur de relever la limite. |
| Type non autorisé | L'administrateur a restreint les types de service autorisés. |

Pour le protocole et la conception, voir le dépôt : `docs/tunnel-protocol.md`, `bridge/README.md` et la décision d'architecture `docs/adr/0020-pc-tunnel.md`.
