---
title: "Appels vocaux"
description: "Parler avec les personnes qui travaillent sur la même page, le même projet ou le même espace, directement depuis le navigateur."
---

Un **appel vocal** permet aux personnes qui peuvent ouvrir une page ou un projet de se parler pendant qu'elles travaillent. L'audio passe **directement entre les participants** (WebRTC) : le serveur Looty met seulement les navigateurs en relation, il n'entend et ne garde rien.

## Lancer ou rejoindre un appel

- Cliquez sur **Lancer un appel** dans l'en-tête d'une page ou d'un projet. Si un appel est déjà en cours, le bouton indique **Rejoindre l'appel · 2** avec le nombre de personnes, et leurs noms au survol.
- Le navigateur demande le micro la première fois. Si vous refusez, la barre d'appel explique comment l'autoriser de nouveau.
- La barre d'appel flotte en bas de l'écran et **reste quand vous changez de page** : continuez à parler en ouvrant un tableau ou un autre document. **Quitter** (ou fermer l'onglet) raccroche.

## Pendant l'appel

- **Couper le micro** : le bouton micro, ou `Ctrl`+`Alt`+`M` (`Cmd`+`Alt`+`M` sur Mac). Les autres voient qui est coupé.
- **Qui parle** est indiqué par un anneau autour de l'avatar.
- **Réglages** (engrenage) : choix du micro et volume de chaque personne. Si le micro est débranché, Looty passe au micro par défaut ou vous invite à en choisir un autre ; vous continuez à écouter.
- Annulation d'écho, suppression de bruit et gain automatique sont actifs. L'audio est en Opus, optimisé pour la voix, avec suppression des silences.

## Limites

- Jusqu'à **6 personnes** par appel par défaut (l'administrateur peut changer cette valeur avec `LOOTY_CALL_MAX_PARTICIPANTS`).
- Mêmes droits que le contenu : qui peut ouvrir la page, le projet ou l'espace peut rejoindre son appel, et personne d'autre.
- Sur des réseaux très restrictifs (certains pare-feu d'entreprise ou d'école), deux personnes peuvent ne pas réussir à se connecter directement. Un administrateur peut activer un serveur relais pour ce cas (voir plus bas).
- La latence est celle d'un chemin réseau direct : en général bien en dessous de 150 ms. Le test automatisé rapporte le temps d'aller-retour mesuré entre ses deux navigateurs.

Pour les choix techniques, voir la décision d'architecture `docs/adr/0019-calls-webrtc-mesh.md`.

## Hébergement : relais (TURN) pour les réseaux restrictifs

Les connexions directes fonctionnent sur la plupart des réseaux domestiques et de campus. Sur les réseaux très restrictifs (NAT symétrique, pare-feu qui bloquent l'UDP), un relais est nécessaire. Il est **désactivé par défaut**.

- **Docker (install.sh)** : `install.sh --turn` (ou `--upgrade --turn` sur une installation existante) ajoute un service `coturn` et un `TURN_SECRET` dans `.env`. Looty propose alors à chaque personne connectée un accès relais temporaire (valable 24 heures, `LOOTY_TURN_TTL`) ; le secret ne quitte jamais le serveur.
- **NixOS** : `services.looty.turn = { enable = true; secretFile = "/run/secrets/looty-turn"; };` et la même valeur dans le fichier d'environnement sous `LOOTY_TURN_SECRET`.
- **Ports à ouvrir** sur le pare-feu du serveur : **3478 en UDP et TCP**, et la plage de relais **49160-49200 en UDP** (`TURN_MIN_PORT` / `TURN_MAX_PORT`). Gardez la plage petite : chaque participant relayé utilise quelques ports, une vingtaine suffit pour une petite instance. Derrière un NAT (serveur cloud), renseignez `TURN_EXTERNAL_IP` avec l'adresse publique.
- **Bande passante** : l'audio relayé coûte environ 32 kbit/s par sens et par participant relayé sur votre serveur. Un appel à 6 où tout le monde est relayé reste sous 1 Mbit/s. Seules les personnes qui ne peuvent pas se connecter directement l'utilisent.
- **Vérification** : Administration > Diagnostic a une ligne « Relais des appels (TURN) ». Utilisez vos propres serveurs STUN avec `LOOTY_STUN_URLS` si vous ne voulez pas du serveur public.
