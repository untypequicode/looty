---
title: "Installer les apps"
description: "Looty sur Android, Linux, Windows, iPhone et iPad, et le travail hors ligne."
---

Tous les téléchargements sont sur le [site de Looty](/fr/#download). Chaque app se connecte à votre instance et garde vos tableaux, documents et agenda sur l’appareil.

## Android

- Téléchargez l’APK depuis le site (Android 8 ou plus) et autorisez l’installation depuis cette source la première fois. L’app n’utilise aucun service Google et se met à jour depuis l’app. Vous préférez F-Droid ? Ajoutez le [dépôt F-Droid de Looty](/fr/docs/use/android-fdroid/) : cette version est mise à jour par F-Droid.
- **Les liens s’ouvrent dans l’app** : un lien `looty://` ou un lien vers votre instance propose de s’ouvrir dans l’app.
- **Widgets** sur l’écran d’accueil, et **partage** : partagez un texte ou un fichier depuis n’importe quelle app vers Looty.
- Les notifications passent par UnifiedPush (voir [notifications](/fr/docs/use/features/notifications/)).
- Pour voir vos agendas Looty dans l’application Agenda du téléphone, voir [agenda](/fr/docs/use/features/calendar/).

## Linux

- Debian et Ubuntu : installez le `.deb` ; il ajoute aussi le dépôt signé, donc `sudo apt update && sudo apt upgrade` garde Looty à jour.
- NixOS et Nix : `nix run github:untypequicode/looty`.
- Flatpak : voir [Flathub et winget](/fr/docs/use/flathub-winget/).

## Windows

Windows 10 et 11. L’installateur est réservé à votre utilisateur. Windows peut signaler un éditeur inconnu : *Informations complémentaires*, puis *Exécuter quand même*. Elle se met à jour toute seule : l’app cherche une nouvelle version toutes les quelques heures et l’installe après avoir vérifié sa signature. Avec winget : voir [Flathub et winget](/fr/docs/use/flathub-winget/).

## iPhone et iPad

Il n’y a pas d’app sur l’App Store. Ouvrez votre instance dans Safari, puis *Partager > Sur l’écran d’accueil* : Looty s’ouvre en plein écran et peut envoyer des notifications.

## Hors ligne

Les apps Android et bureau gardent vos données sur l’appareil. Vous pouvez lire et modifier sans connexion ; les documents fusionnent à la reconnexion. Les fichiers Moodle peuvent aussi être conservés hors ligne (voir [Moodle](/fr/docs/use/features/moodle/)).
