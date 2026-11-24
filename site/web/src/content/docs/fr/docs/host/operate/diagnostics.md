---
title: "Diagnostiquer son instance"
description: "Vérifier que chaque service tourne et lire les journaux."
---

```sh
docker compose ps                         # tous les services doivent tourner (postgres healthy)
docker compose logs --tail 100 backend    # API, connexion, e-mails, migrations
docker compose logs --tail 100 collaboration frontend
curl -s https://<domaine>/api/health      # {"version": "..."} quand le backend répond
```

La page de connexion indique quand le fournisseur OpenID Connect est injoignable ou a rejeté Looty ; le journal du backend nomme alors la cause (mauvais issuer, mauvais secret, URL de redirection inconnue). Mettez `RUST_LOG=debug` sur le backend pour plus de détails le temps de l’enquête.
