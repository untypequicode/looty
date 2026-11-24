---
title: "Flathub et winget"
description: "Installer l'app de bureau Looty depuis Flathub sous Linux ou avec winget sous Windows."
---

L’app de bureau est empaquetée pour deux gestionnaires de paquets. Les deux installent la même app que les téléchargements du [site](/fr/#download).

:::note
Ces listes sont en cours de mise en place et ne sont peut-être pas encore disponibles. En attendant, utilisez le `.deb`, Nix ou l’installateur Windows de [Installer les apps](/fr/docs/use/apps/).
:::

## Linux : Flathub

```sh
flatpak install flathub io.github.untypequicode.looty
flatpak run io.github.untypequicode.looty
```

- Flathub met l’app à jour : `flatpak update`. La vérification de mise à jour intégrée à l’app n’est pas utilisée.
- Les permissions sont minimales : réseau (pour joindre votre serveur), notifications, icône de la barre système et trousseau du système. Les téléchargements et *Enregistrer sous* passent par la fenêtre de fichiers du système : Looty ne peut pas lire vos autres dossiers.
- Le raccourci global d’ajout rapide et les liens `looty://` dépendent de votre environnement de bureau.

## Windows : winget

```powershell
winget install untypequicode.Looty
winget upgrade untypequicode.Looty
```

- L’installateur est le même installateur par utilisateur que sur le site. Il n’est pas encore signé : SmartScreen peut signaler un éditeur inconnu ; winget affiche l’empreinte de l’installateur et la vérifie avant de le lancer.
- `winget upgrade` installe les nouvelles versions.

## Pour les empaqueteurs

Les manifestes sont générés depuis ce dépôt. Les étapes du mainteneur (soumission à Flathub et au catalogue winget) sont dans le [guide d’empaquetage](https://github.com/untypequicode/looty/blob/main/docs/packaging-flathub-winget.md).
