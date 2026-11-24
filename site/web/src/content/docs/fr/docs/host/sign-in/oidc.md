---
title: "OpenID Connect (recommandé)"
description: "Comment Looty connecte les gens avec un fournisseur OpenID Connect."
---

Looty connecte les gens par **OpenID Connect (OIDC)** : le fournisseur d’identité de votre organisation, ou un que vous faites tourner. C’est la méthode recommandée.

## Ce que Looty attend du fournisseur

- Un **client** pour Looty (confidentiel, avec un secret), en flux « code d’autorisation » avec **PKCE**.
- Les portées `openid email profile` : Looty a besoin d’une **adresse e-mail** pour chaque compte.
- URL de redirection (callback) : `<BASE_URL>/api/auth/callback/<OIDC_PROVIDER_ID>`. Fixez `OIDC_PROVIDER_ID` à un UUID (`uuidgen`) dans `.env` pour la connaître **avant** le premier démarrage.

## Réglages

```sh
OIDC_ISSUER=https://id.example.org
OIDC_CLIENT_ID=looty
OIDC_CLIENT_SECRET=...
OIDC_PROVIDER_ID=<uuid>   # facultatif mais pratique
```

Looty lit ces variables **seulement à son tout premier démarrage** ; ensuite, le fournisseur se modifie dans *Administration > Fournisseurs d’authentification*. Le premier compte qui se connecte devient l’administrateur. Un fournisseur dont le certificat vient d’une autorité interne demande `OIDC_CA_FILE`.

## Guides par fournisseur

[Zitadel](/fr/docs/host/sign-in/zitadel/) · [Keycloak](/fr/docs/host/sign-in/keycloak/) · [Authentik](/fr/docs/host/sign-in/authentik/) · [Google](/fr/docs/host/sign-in/google/) · [Pocket ID inclus](/fr/docs/host/sign-in/pocket-id/) · [Comptes locaux](/fr/docs/host/sign-in/local-accounts/)
