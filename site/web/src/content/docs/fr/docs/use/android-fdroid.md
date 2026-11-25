---
title: "Android : le dépôt F-Droid de Looty"
description: "Installer et mettre à jour Looty sur Android avec F-Droid ou Droid-ify, depuis le dépôt de Looty."
---

Looty publie son propre dépôt [F-Droid](https://f-droid.org). Ajoutez-le une fois dans F-Droid, Droid-ify ou Neo Store, et Looty s’installe et se met à jour comme n’importe quelle autre app. Cette version n’a **pas de mise à jour intégrée** et ne demande pas la permission d’installer des applications : c’est votre client F-Droid qui met à jour. C’est par ailleurs la même app que l’APK de la [page de téléchargement](/fr/#download), avec le même identifiant et la même clé de signature : vous pouvez passer de l’une à l’autre sans perdre vos données.

Looty n’est pas encore dans le catalogue principal de F-Droid. En attendant, utilisez ce dépôt.

## Ajouter le dépôt

Sur votre téléphone, ouvrez ce lien, ou scannez le QR code avec le client F-Droid :

[fdroidrepos://looty.untypequicode.fr/fdroid/repo](fdroidrepos://looty.untypequicode.fr/fdroid/repo)

![QR code du dépôt F-Droid de Looty](/fdroid/repo/index.png)

Si vous saisissez l’adresse vous-même, c’est `https://looty.untypequicode.fr/fdroid/repo`.

## Vérifier la clé

F-Droid affiche l’empreinte du dépôt quand vous l’ajoutez. Elle doit être celle publiée sur ce site, l’empreinte SHA-256 de la clé de signature du dépôt :

`98AB977C06DF41102B93943862203BDFABF197A589B68955A3138882FB6D218C`

[Empreinte de la clé du dépôt](/fdroid/fingerprint.txt)

Le QR code ci-dessus la contient déjà : le scanner est la façon la plus sûre d’ajouter le dépôt. Si l’empreinte est différente, n’ajoutez pas le dépôt.

## Mises à jour

Le dépôt ne garde que la dernière version. Dans F-Droid, tirez pour actualiser, ou activez les mises à jour automatiques pour ce dépôt. L’écran *Mises à jour de l’application* de Looty n’affiche alors que les nouveautés de chaque version.

## Le construire soi-même

La version F-Droid se construit avec `./gradlew assembleFdroidRelease` dans le dossier `android/` du [code source](https://github.com/untypequicode/looty). Elle n’a aucune dépendance propriétaire, et deux constructions d’une même version donnent le même APK non signé.
