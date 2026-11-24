---
title: "Comptes locaux (déconseillés)"
description: "Comptes e-mail et mot de passe facultatifs, avec double authentification, et pourquoi OpenID Connect reste préféré."
---

Looty préfère **OpenID Connect** : le stockage des mots de passe, les réinitialisations et les seconds facteurs sont mieux gérés par un fournisseur d’identité dédié. Si vous n’en avez pas, le plus simple est le [Pocket ID inclus](/fr/docs/host/sign-in/pocket-id/).

Les comptes locaux existent pour les installations où c’est trop. Ils sont **désactivés par défaut** :

```sh
LOOTY_LOCAL_ACCOUNTS=true    # les active, en plus d’OpenID Connect si vous l’avez aussi
LOOTY_LOCAL_SIGNUP=closed    # closed (défaut : un administrateur invite) ou open
```

## Comportement

- **Premier compte.** Sur une instance vide, le premier compte devient l’administrateur. Pour qu’un inconnu ne le prenne pas avant vous, la page de connexion demande un **code d’installation** affiché dans les journaux du backend au démarrage : `docker compose logs backend | grep "setup"`.
- **L’inscription est fermée.** Les administrateurs invitent dans *Administration > Comptes locaux* : la personne reçoit un lien (par e-mail si le SMTP est configuré, sinon vous le copiez) pour choisir un mot de passe. Avec `LOOTY_LOCAL_SIGNUP=open`, tout le monde peut créer un compte et doit confirmer son adresse e-mail (ou un administrateur la confirme).
- **Mots de passe** hachés en argon2id, d’au moins 12 caractères, comparés hors ligne à une liste de mots de passe très courants.
- **Les tentatives sont limitées** par compte et par adresse IP : cinq échecs verrouillent le compte 15 minutes, avec la même réponse que le compte existe ou non. Placez Looty derrière un proxy qui renseigne `X-Forwarded-For`, sinon la limite par adresse ne voit que le proxy.
- **Mot de passe oublié.** Avec le SMTP, un lien valable une heure est envoyé. Sans lui, un administrateur copie un lien de réinitialisation depuis *Comptes locaux*. La réinitialisation ferme les autres sessions de la personne.
- **Double authentification (TOTP).** Toute application d’authentification convient. Elle est **obligatoire pour les administrateurs** : un administrateur qui n’en a pas peut saisir son mot de passe mais reste bloqué sur l’écran de configuration. Les administrateurs peuvent l’exiger de tout le monde, et la réinitialiser pour quelqu’un qui a perdu son appareil. Chaque personne reçoit aussi 10 codes de secours à usage unique. Les applications de bureau et Android se connectent par le navigateur : l’étape du code est la même.

:::caution
`LOOTY_DEV_LOGIN=true` est un interrupteur de **développement** qui permet à n’importe qui de se connecter sous n’importe quelle adresse e-mail. Ce n’est pas une fonction de comptes locaux. **Ne l’activez jamais sur une vraie instance.**
:::
