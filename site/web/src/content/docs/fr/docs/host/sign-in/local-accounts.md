---
title: "Comptes locaux (déconseillés)"
description: "Pourquoi Looty n’a pas de mots de passe intégrés, et quoi utiliser à la place."
---

Looty n’a **pas de comptes à mot de passe intégrés** : on se connecte toujours par un fournisseur OpenID Connect. C’est voulu : Looty ne stocke ni mots de passe, ni procédures de réinitialisation, ni seconds facteurs, et un fournisseur d’identité dédié le fait mieux.

Si vous n’avez pas de fournisseur, **utilisez le [Pocket ID inclus](/fr/docs/host/sign-in/pocket-id/)** : quelques lignes de plus dans le même fichier compose, et vous avez des passkeys.

:::danger
`LOOTY_DEV_LOGIN=true` est un interrupteur de **développement** qui permet à n’importe qui de se connecter sous n’importe quelle adresse e-mail. Ce n’est pas une fonction de comptes locaux. **Ne l’activez jamais sur une vraie instance.**
:::
