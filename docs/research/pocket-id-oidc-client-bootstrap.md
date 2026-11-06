# Pocket ID : créer le client OIDC de Looty au premier démarrage

Recherche pour l'issue #161 (« Self-host : fournisseur OIDC inclus en option, Pocket ID préconfiguré »).
Date : 2026-11-06. Version de référence : **Pocket ID v2.19.2** (tag `v2.19.2`, commit `2b2750e`), docs du dépôt `pocket-id/website` au commit `ac8c2fe`, module NixOS de nixpkgs au commit `9b88457`.

Les sections 1 à 6 ne contiennent que des **faits sourcés**. La section 7 est la **recommandation**, séparée des faits.

Abréviations des liens :
- `PID` = `https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6`
- `DOCS` = `https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs`
- `NIX` = `https://github.com/NixOS/nixpkgs/blob/9b8845711b67c9ea6674afcf853692a09c5dd120/nixos/modules/services/security/pocket-id.nix`

## En bref

- **Aucun fichier de configuration et aucune variable ne déclarent un client OIDC.** Pocket ID ne propose pas de « seed » de clients (rien dans `backend/internal` hormis le seed des tests e2e). L'import (`pocket-id import`) remplace toute la base et est expérimental : ce n'est pas un mécanisme d'amorçage.
- **Il y a en revanche tout ce qu'il faut pour le faire par l'API, sans aucune action humaine** :
  - `STATIC_API_KEY` (depuis la **v2.2.0**) : une clé API admin fixe, fournie par variable d'environnement, qui marche **avant même que le premier compte existe** ;
  - `POST /api/oidc/clients` accepte un **`id` choisi** (depuis la **v1.8.0**) ;
  - `POST /api/oidc/clients/{id}/secrets` accepte un **secret choisi** (≥ 16 caractères ; secret choisi depuis la **v2.12.0**, route au pluriel `/secrets` depuis la **v2.14.0**).
  - Looty peut donc générer `OIDC_CLIENT_ID` et `OIDC_CLIENT_SECRET` d'avance (dans `.env`), et un conteneur d'amorçage n'a qu'à les « pousser » dans Pocket ID. Rien n'a besoin d'être relu.
- **Pas de sous-chemin** : `APP_URL` est refusé s'il contient un chemin. Il faut un (sous-)domaine dédié, par exemple `auth.<domaine>`.
- **Le premier compte Pocket ID se crée sur `/setup`** (route publique `POST /api/signup/setup`), puis on enregistre une passkey. C'est interactif ; la page reste ouverte à n'importe qui tant qu'aucun compte n'existe.
- **Aucune claim OIDC ne dit si l'utilisateur est admin de Pocket ID.** Seul `groups` (scope `groups`) est disponible. Ce n'est pas un problème : Looty fait déjà du premier compte connecté son administrateur.

## 1. Mécanisme déclaratif ou variables pour créer des clients au premier démarrage

**Faits**

- La liste complète des variables lues par Pocket ID est la struct `EnvConfigSchema` ([`PID/backend/internal/common/env_config.go#L58-L160`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go#L58-L160)). Aucune ne décrit un client OIDC. Les réglages de l'interface (« Application Configuration ») peuvent passer en variables avec `UI_CONFIG_DISABLED=true`, mais aucun ne crée de client non plus ([`DOCS/configuration/environment-variables.md#L212-L242`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/configuration/environment-variables.md#L212-L242)).
- `STATIC_API_KEY` : « An API key of at least 16 characters with admin rights, for declarative setups. It acts as a user named "Static API User". » ([`DOCS/configuration/environment-variables.md#L60`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/configuration/environment-variables.md#L60)). Elle accepte aussi `STATIC_API_KEY_FILE` ([même page, L10](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/configuration/environment-variables.md#L10)).
  - Déclaration et longueur minimale dans le code : [`env_config.go#L75`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go#L75) et [`#L246-L248`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go#L246-L248).
  - La clé est comparée en clair à la valeur de la variable ; si elle correspond, Pocket ID crée à la volée un utilisateur `00000000-0000-0000-0000-000000000000` avec `IsAdmin: true` ([`PID/backend/internal/apikey/service.go#L158-L165`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/apikey/service.go#L158-L165), [`#L223-L261`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/apikey/service.go#L223-L261), ID fixe dans [`common/reserved.go#L3-L5`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/reserved.go#L3-L5)).
  - Si la variable est retirée, cet utilisateur synthétique est supprimé au démarrage suivant ([`apikey/service.go#L25-L38`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/apikey/service.go#L25-L38), [`#L263`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/apikey/service.go#L263)).
  - Cet utilisateur **ne compte pas** comme « premier compte » : la page `/setup` reste disponible ([`PID/backend/internal/usersignup/service.go#L244-L251`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/usersignup/service.go#L244-L251), `Where("id != ?", common.StaticApiKeyUserID)`).
  - **Version d'introduction : v2.2.0**, commit [`746aa71`](https://github.com/pocket-id/pocket-id/commit/746aa71d67299bf82685ff50696a82bebce8b409) / PR [#1229](https://github.com/pocket-id/pocket-id/pull/1229) : « This variable can be useful if you need API access before the first user has been created for example with declarative installations. » Entrée du changelog : [`PID/CHANGELOG.md#L740-L760`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/CHANGELOG.md#L740-L760).
- La demande de « client secret déclaratif » ([issue #1574](https://github.com/pocket-id/pocket-id/issues/1574), suite de [#1166](https://github.com/pocket-id/pocket-id/issues/1166)) a été close le 2026-07-26 par le commit [`9f55978`](https://github.com/pocket-id/pocket-id/commit/9f559788) « feat: add support for declaritive client secret configuration (#1619) », sorti en **v2.12.0**. La solution retenue passe par l'API, pas par une configuration statique.
- Export et import : `pocket-id import` « will erase all existing data » et la fonction est marquée « experimental » ([`DOCS/setup/data-export-import.md`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/setup/data-export-import.md)). Ce n'est pas utilisable pour ajouter un client.

## 2. API REST de création d'un client OIDC

**Authentification**

- En-tête `X-API-Key` ([`PID/backend/internal/middleware/authenticator.go#L20`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/middleware/authenticator.go#L20)). La doc l'écrit `X-API-KEY` ([`DOCS/api.md#L21-L24`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/api.md#L21-L24)) ; les en-têtes HTTP ne tiennent pas compte de la casse.
- Il y a deux façons d'obtenir une clé :
  - dans l'interface (Administration → API Keys), ce qui suppose un admin connecté, donc une action manuelle ([`DOCS/api.md#L9-L17`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/api.md#L9-L17)) ;
  - sans interaction, par `STATIC_API_KEY` (§1). C'est la seule voie non interactive.
- Les routes sont protégées par les scopes `oidc-clients:read` et `oidc-clients:write` ([`PID/backend/internal/controller/oidc_controller.go#L31-L41`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/oidc_controller.go#L31-L41)). La clé statique agit comme un utilisateur admin (§1).

**Routes utiles** ([`oidc_controller.go#L31-L41`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/oidc_controller.go#L31-L41))

| Méthode et chemin | Rôle |
| --- | --- |
| `GET /api/oidc/clients/{id}` | Lire un client. Renvoie 404 (`NotFound("OIDC client")`, [`apperror/constructors.go#L14-L16`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/apperror/constructors.go#L14-L16)) s'il n'existe pas (`getClientInternal` dans [`service/oidc_service.go`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/service/oidc_service.go)) |
| `POST /api/oidc/clients` | Créer un client (réponse 201, `OidcClientCreatedDto`) |
| `PUT /api/oidc/clients/{id}` | Mettre à jour un client |
| `GET /api/oidc/clients/{id}/secrets` | Lister les secrets (sans leur valeur ; seuls l'ID et un préfixe de 4 caractères sont visibles) |
| `POST /api/oidc/clients/{id}/secrets` | Ajouter un secret, généré ou **fourni** |
| `DELETE /api/oidc/clients/{id}/secrets/{secretId}` | Supprimer un secret |

**Corps de `POST /api/oidc/clients`** : `OidcClientCreateDto` = `OidcClientUpdateDto` plus `id` ([`PID/backend/internal/dto/oidc_dto.go#L52-L77`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/dto/oidc_dto.go#L52-L77)). Champs principaux :

- `name` (obligatoire, 50 caractères au plus), `description` ;
- `id` : facultatif, 2 à 128 caractères, validé par `client_id` ([L74-L77](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/dto/oidc_dto.go#L74-L77)). **On peut donc choisir l'ID du client**, depuis la **v1.8.0** (commit [`a5efb95`](https://github.com/pocket-id/pocket-id/commit/a5efb950), PR [#864](https://github.com/pocket-id/pocket-id/pull/864)). Si l'ID existe déjà, la réponse est **409** `ClientIDAlreadyExists` ([`apperror/constructors.go#L263-L265`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/apperror/constructors.go#L263-L265)) ;
- `callbackURLs`, `logoutCallbackURLs` : listes validées par `callback_url_pattern`, jokers acceptés (L55-L56) ;
- `isPublic`, `pkceEnabled`, `requiresReauthentication`, `requiresPushedAuthorizationRequests`, `skipConsent`, `launchURL`, `logoUrl`/`darkLogoUrl`, `isGroupRestricted`, `accessTokenDurationMinutes`, `refreshTokenDurationMinutes`, `backchannelLogoutURL` ;
- `credentials.secrets` est **en lecture seule** : un secret envoyé dans ce champ est ignoré ([`oidc_dto.go`, `OidcClientCredentialsDto`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/dto/oidc_dto.go)).

**Secret du client**

- À la création, si le réglage `AUTO_CREATE_OIDC_CLIENT_SECRET` vaut `true` (valeur par défaut) et que le client est confidentiel, Pocket ID génère un secret aléatoire de 32 caractères et le renvoie **une seule fois**, dans `createdSecret.secret` ([`controller/oidc_controller.go#L138-L171`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/oidc_controller.go#L138-L171), [`service/oidc_service.go#L156-L183`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/service/oidc_service.go#L156-L183), [`dto/oidc_dto.go#L42-L45`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/dto/oidc_dto.go#L42-L45)). Réglage documenté dans [`DOCS/configuration/environment-variables.md#L231`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/configuration/environment-variables.md#L231) ; comportement rétabli en v2.17.0 (commit [`d259a15`](https://github.com/pocket-id/pocket-id/commit/d259a151)). Entre v2.14 et v2.16, aucun secret n'était créé automatiquement.
- `POST /api/oidc/clients/{id}/secrets` avec `{"secret": "<valeur>", "expiresAt": null}` : `secret` est facultatif, mais s'il est fourni il doit faire au moins 16 caractères ASCII imprimables ([`dto/oidc_dto.go#L90-L96`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/dto/oidc_dto.go#L90-L96)). Sans `secret`, Pocket ID en génère un ([`service/oidc_service.go#L452-L471`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/service/oidc_service.go#L452-L471)). Les secrets sont stockés hachés (SHA-256) avec un préfixe de 4 caractères en clair. Un client public est refusé ([L420](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/service/oidc_service.go#L420)), et un client a 20 secrets au plus ([`model/oidc.go#L41-L43`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/model/oidc.go#L41-L43)).
- **Attention aux versions** : jusqu'à la v2.13 la route était au singulier (`POST /api/oidc/clients/:id/secret`, corps `OidcClientSecretDto{secret}`). Elle passe au pluriel `/secrets`, avec plusieurs secrets par client, en **v2.14.0** (commit [`155a1fc`](https://github.com/pocket-id/pocket-id/commit/155a1fcb), PR [#1679](https://github.com/pocket-id/pocket-id/pull/1679) ; [`CHANGELOG.md#L222-L243`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/CHANGELOG.md#L222-L243)). **Le script d'amorçage dépend donc de la version** : il faut épingler Pocket ID à **v2.14 ou plus**.

**URL de rappel (callback)**

- Il faut au moins une URL de rappel : Pocket ID « doesn't save the first URL an app sends anymore » ([`DOCS/advanced/callback-url-wildcards.md#L7-L13`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/advanced/callback-url-wildcards.md#L7-L13)). L'enregistrement automatique à la première utilisation (TOFU) a été retiré en v2.10.0 (commit [`931a6c2`](https://github.com/pocket-id/pocket-id/commit/931a6c2adb39fb8cdfe18487f4206f8848b3d026)).
- Le joker `*` couvre un segment de chemin (exemple : `https://example.com/oauth/*/callback`) ([même page, L25-L44](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/advanced/callback-url-wildcards.md#L25-L44)). La doc conseille d'utiliser des URL exactes quand c'est possible (L19-L21).
- `ALLOW_INSECURE_CALLBACK_URLS` vaut `true` par défaut, « until next major release » ([`env_config.go#L179`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go#L179) ; [`DOCS/configuration/environment-variables.md#L58`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/configuration/environment-variables.md#L58)).

**Compatibilité avec le client OIDC de Looty** : la découverte annonce `token_endpoint_auth_methods_supported: client_secret_basic, client_secret_post, none` et `code_challenge_methods_supported: plain, S256` ([`PID/backend/internal/controller/well_known_controller.go#L100-L122`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/well_known_controller.go#L100-L122)). Looty utilise le code d'autorisation, PKCE S256 et un secret client (voir §8). Ça correspond.

## 3. Premier compte (admin) de Pocket ID

**Faits**

- Doc : « Open `https://id.example.com/setup` and fill in your username, email address and name. After **Sign Up**, Pocket ID asks you to set up a passkey […]. The setup page only works until the first account exists. » ([`DOCS/setup/installation.mdx#L86-L95`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/setup/installation.mdx#L86-L95)).
- Code : `GET` et `POST /api/signup/setup` sont **publics** (`r.Public()`) ([`PID/backend/internal/usersignup/module.go#L111-L112`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/usersignup/module.go#L111-L112)).
  - Le corps est `{username, email?, firstName, lastName}` ([`usersignup/dto.go#L9-L15`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/usersignup/dto.go#L9-L15)).
  - L'utilisateur est créé avec `IsAdmin: true`, sous verrou, et seulement si aucun compte réel n'existe. Pocket ID ouvre ensuite une session ([`usersignup/service.go#L175-L215`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/usersignup/service.go#L175-L215)).
  - **Ni mot de passe ni passkey à cette étape** : la passkey s'ajoute ensuite, dans le navigateur. On peut techniquement appeler l'API en script, mais le compte n'aurait alors aucun moyen de connexion sans passkey ni lien de connexion unique.
- Sécurité (constat tiré du code) : aucune variable ne protège `/setup`. Tant que personne n'a créé le premier compte, **la première personne qui ouvre `https://auth.<domaine>/setup` devient admin de Pocket ID**.
- La commande `pocket-id one-time-access-token <username|email>` crée un lien de connexion à usage unique, valable 1 heure, **pour un utilisateur qui existe déjà**. Elle écrit directement dans la base et affiche `…/lc/<token>` ([`PID/backend/internal/cmds/one_time_access_token.go#L21-L70`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/cmds/one_time_access_token.go#L21-L70)). Elle ne crée pas de compte. Avec Docker : `docker compose exec pocket-id /app/pocket-id one-time-access-token <user>`.
- Inscriptions : `ALLOW_USER_SIGNUPS` vaut `disabled` par défaut, sinon `withToken` ou `open` ([`DOCS/configuration/environment-variables.md#L240`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/configuration/environment-variables.md#L240)). `EMAILS_VERIFIED` vaut `false` par défaut (L239). Ces deux réglages ne sont lus depuis l'environnement qu'avec `UI_CONFIG_DISABLED=true` (L212-L216).

## 4. Variables d'environnement utiles

Source : [`DOCS/configuration/environment-variables.md`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/configuration/environment-variables.md) et [`env_config.go`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go).

| Variable | Fait |
| --- | --- |
| `APP_URL` | Adresse publique, par défaut `http://localhost:1411`. « without a path. Passkeys are bound to its domain » (doc L17). **Le code refuse un chemin** : `"%s must not contain a path"` ([`env_config.go#L398`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go#L398), [`#L411-L420`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go#L411-L420)). **Pas de sous-chemin possible**, il faut un (sous-)domaine dédié. `APP_URL` est aussi l'`issuer` de la découverte ([`well_known_controller.go#L100`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/well_known_controller.go#L100)). |
| `PUBLIC_APP_URL` | Ancien nom (v0.x), **abandonné**. Le module NixOS refuse cette option et renvoie au guide de migration v1 ([`NIX#L150-L171`](https://github.com/NixOS/nixpkgs/blob/9b8845711b67c9ea6674afcf853692a09c5dd120/nixos/modules/services/security/pocket-id.nix#L150-L171)). |
| `INTERNAL_APP_URL` | Base des endpoints token, userinfo, introspection et JWKS dans la découverte, « for apps that reach Pocket ID at another address than browsers » (doc L34). Ne doit pas avoir de chemin non plus. `issuer` et `authorization_endpoint` restent sur `APP_URL` ([`well_known_controller.go#L100-L108`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/well_known_controller.go#L100-L108)). |
| `ENCRYPTION_KEY` (`_FILE`) | **Obligatoire**, 16 octets au moins ([`env_config.go#L231-L233`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go#L231-L233)). La doc conseille `openssl rand -base64 32` ; avec `_FILE`, un saut de ligne final fait partie de la clé (doc L143-L153). |
| `TRUST_PROXY` | `false` par défaut. Prend une liste d'IP ou de CIDR, ou `true` pour faire confiance à toutes les adresses (doc L170-L176). |
| `DB_CONNECTION_STRING` (`_FILE`) | SQLite `data/pocket-id.db` par défaut. Une URL `postgres://` choisit PostgreSQL ; « Only one Pocket ID instance may use a database at a time » (doc L117-L141). |
| `STATIC_API_KEY` (`_FILE`) | Voir §1. |
| `ALLOW_INSECURE_CALLBACK_URLS` | `true` par défaut ; à mettre à `false` derrière HTTPS (doc L58). |
| `UI_CONFIG_DISABLED` | Lit les réglages de l'interface depuis l'environnement (doc L110, L212-L219). |
| `PORT` | `1411` par défaut (doc L27). |

Proxy : Pocket ID doit être servi en HTTPS, sauf sur `localhost`, à cause des passkeys ([`DOCS/setup/reverse-proxy.mdx#L10-L22`](https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/setup/reverse-proxy.mdx#L10-L22)). La doc fournit des exemples Caddy (`id.example.com { reverse_proxy pocket-id:1411 }`) et Traefik (étiquettes sur le service) dans la même page.

Image Docker : `pocketid/pocket-id:v2` ou `ghcr.io/pocket-id/pocket-id:v2`. Le compose officiel prévoit une vérification de santé `["CMD", "/app/pocket-id", "healthcheck"]` (elle interroge `/healthz`) et le volume `./data:/app/data` ([`PID/docker-compose.yml`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/docker-compose.yml)). Une variante `-distroless` existe, sans shell ([`PID/docker/Dockerfile-distroless`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/docker/Dockerfile-distroless)).

## 5. Claims et « admin »

**Faits** ([`PID/backend/internal/oidc/claims_service.go#L152-L190`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/oidc/claims_service.go#L152-L190))

- Toujours : `sub` (ID utilisateur Pocket ID, un UUID).
- Scope `profile` : `given_name`, `family_name`, `name`, `display_name`, `preferred_username`, `picture`, plus les **custom claims** de l'utilisateur et de ses groupes.
- Scope `email` : `email` et `email_verified`, seulement si l'utilisateur a une adresse.
- Scope `groups` : `groups`, la liste des **noms** de groupes.
- `claims_supported` dans la découverte : `sub, given_name, family_name, name, display_name, email, email_verified, preferred_username, picture, groups, auth_time, amr` ([`well_known_controller.go#L111`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/well_known_controller.go#L111)).
- **Aucune claim OIDC n'indique « admin de Pocket ID ».** La claim `isAdmin` n'existe que dans le jeton de **session interne** de Pocket ID (`GenerateAccessToken`, signé avec la clé de session, utilisé par webauthn et le signup), pas dans les ID tokens ni dans userinfo ([`PID/backend/internal/service/jwt_service.go#L28-L30`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/service/jwt_service.go#L28-L30), [`#L322-L352`](https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/service/jwt_service.go#L322-L352)).
- Note : sur `main` (pas encore publié), les claims passent par une « claim mapping policy » intégrée ([`main:backend/internal/claimmappingpolicy/bootstrap.go`](https://github.com/pocket-id/pocket-id/blob/23d7968ccdcbe2343d2e025187f226ded9aa10ea/backend/internal/claimmappingpolicy/bootstrap.go#L17-L31)). On y trouve les mêmes claims, toujours sans champ admin.

## 6. Module NixOS `services.pocket-id` (nixpkgs)

**Faits** ([`NIX`](https://github.com/NixOS/nixpkgs/blob/9b8845711b67c9ea6674afcf853692a09c5dd120/nixos/modules/services/security/pocket-id.nix) ; paquet en 2.19.1 sur `nixos-unstable`, [`package.nix`](https://github.com/NixOS/nixpkgs/blob/df4f7858694c1966d0b113f56164d7c855d0e42b/pkgs/by-name/po/pocket-id/package.nix))

- Options : `enable`, `package`, `environmentFile` (par défaut `/dev/null`), `credentials` (variable sans le suffixe `_FILE` → chemin, chargé par `LoadCredential` et `systemd-creds`), `settings` (freeform clé-valeur, écrit **en clair dans le store Nix**), `dataDir` (`/var/lib/pocket-id`), `user`, `group` ([L45-L147](https://github.com/NixOS/nixpkgs/blob/9b8845711b67c9ea6674afcf853692a09c5dd120/nixos/modules/services/security/pocket-id.nix#L45-L147)).
- `settings` déclare `APP_URL` (par défaut `http://localhost`), `TRUST_PROXY` (**type `bool`**, alors que Pocket ID accepte aussi une liste d'IP) et `ANALYTICS_DISABLED` (L86-L126).
- Le module avertit si `ENCRYPTION_KEY`, `MAXMIND_LICENSE_KEY`, `SMTP_PASSWORD` ou `LDAP_BIND_PASSWORD` sont mis dans `settings` (L173-L187). `STATIC_API_KEY` ne figure pas dans cette liste, mais il faut la traiter pareil : la passer par `credentials.STATIC_API_KEY` ou `environmentFile`.
- **Rien pour les clients OIDC** : pas d'option `clients`, pas de service d'amorçage. Une seule unité, `pocket-id.service` (L209-L289), avec `ProtectSystem=strict` et `WorkingDirectory=dataDir`.

## 7. Recommandation pour Looty (avis, distinct des faits ci-dessus)

### 7.1 Ce que Looty fait déjà (constaté dans ce dépôt)

- Au démarrage, `seed_default_provider` crée **un** fournisseur « SSO » à partir de `OIDC_ISSUER`, `OIDC_CLIENT_ID` et `OIDC_CLIENT_SECRET`, si la table `oauth_providers` est vide et que les trois variables sont remplies (`backend/src/main.rs`, `seed_default_provider`). Pocket ID n'est **pas contacté** à ce moment : la découverte se fait à la première connexion (`get_oidc_client`, `backend/src/auth.rs`).
- L'URL de rappel est `{BASE_URL}/api/auth/callback/{provider_id}`, où `provider_id` est l'**UUID aléatoire** de la ligne `oauth_providers` (`backend/src/auth.rs`, `get_oidc_client`). On ne la connaît donc pas avant le premier démarrage de Looty.
- Les scopes demandés sont `openid profile email`, avec PKCE S256 et un secret client (`begin_login`).
- **Premier compte Looty = super admin** : `resolve_user` crée l'utilisateur avec `is_super_admin = (user_count == 0)`. Une adresse e-mail est obligatoire (`missing_email` sinon). `email_verified` ne sert qu'à rattacher un compte existant, pas à créer le premier.
- Les modèles d'installation (`deploy/templates/`) couvrent **Caddy** et **Traefik** (blocs `#@if caddy` / `#@if traefik`, rendus par `deploy/render.sh` et par le générateur du site, ADR 0005). Il n'y a **pas encore** de modèle NixOS pour Looty.

### 7.2 Conception recommandée : un conteneur d'amorçage avec une clé statique

1. **Le générateur** (zip ou `install.sh`) ajoute à `.env`, quand l'option Pocket ID est cochée :
   - `AUTH_DOMAIN=auth.<domaine>` ;
   - `POCKET_ID_ENCRYPTION_KEY` (aléatoire) et `POCKET_ID_STATIC_API_KEY` (aléatoire, 32 caractères ou plus) ;
   - `OIDC_ISSUER=https://auth.<domaine>`, `OIDC_CLIENT_ID=looty` et `OIDC_CLIENT_SECRET` (aléatoire, 32 caractères ou plus).
   Ces secrets restent générés sur la machine, comme le veut l'ADR 0005.
2. **Le service `pocket-id`**, avec une image épinglée **≥ v2.14** (routes `/secrets`) :
   - `APP_URL=https://${AUTH_DOMAIN}`, `ENCRYPTION_KEY`, `STATIC_API_KEY`, `TRUST_PROXY` (réseau du proxy), `ALLOW_INSECURE_CALLBACK_URLS=false` ;
   - le volume `./data/pocket-id:/app/data` (SQLite, plus simple que de partager le Postgres de Looty) ;
   - la vérification de santé `/app/pocket-id healthcheck`.
3. **Le routage** se fait sur un sous-domaine, puisque le sous-chemin est impossible (§4). Avec Caddy, un deuxième bloc `{$AUTH_DOMAIN} { reverse_proxy pocket-id:1411 }`. Avec Traefik, une étiquette `Host(\`${AUTH_DOMAIN}\`)` sur le service.
4. **Le service `pocket-id-init`**, qui tourne une fois (`restart: "no"`, `depends_on: pocket-id: condition: service_healthy`), utilise une petite image avec `sh` et `curl`, car l'image distroless n'a pas de shell. Il parle à `http://pocket-id:1411` par le réseau Docker et doit être **idempotent** :
   - `GET /api/oidc/clients/looty` : 200 → passer à l'étape des secrets ; 404 → créer le client ;
   - `POST /api/oidc/clients` avec `{"id":"looty","name":"Looty","callbackURLs":["https://<APP_DOMAIN>/api/auth/callback/*"],"logoutCallbackURLs":[],"isPublic":false,"pkceEnabled":true,"launchURL":"https://<APP_DOMAIN>"}` ; 409 → considérer le client comme déjà créé ;
   - `GET /api/oidc/clients/looty/secrets`, puis ajouter le secret seulement si aucun préfixe ne correspond aux 4 premiers caractères de `OIDC_CLIENT_SECRET` : `POST /api/oidc/clients/looty/secrets {"secret":"$OIDC_CLIENT_SECRET"}`. Le secret créé automatiquement au moment de la création n'est pas connu ; on peut le supprimer avec `DELETE …/secrets/{id}`, ou désactiver la création avec `AUTO_CREATE_OIDC_CLIENT_SECRET=false`, ce qui suppose `UI_CONFIG_DISABLED=true` et donc de figer toute la configuration de l'interface : déconseillé.
   - Sortie non nulle et message clair en cas d'échec (`docker compose logs pocket-id-init`).
5. **Looty** ne change pas pour l'essentiel : `seed_default_provider` crée le fournisseur à partir des variables, et le client existe déjà dans Pocket ID quand l'utilisateur clique sur « Se connecter ».
6. **Le premier compte** : l'opérateur ouvre `https://auth.<domaine>/setup`, crée son compte et sa passkey, puis se connecte à Looty, dont il devient super admin, car c'est le premier compte Looty. Comme `ALLOW_USER_SIGNUPS=disabled` par défaut, personne d'autre ne peut se connecter avant que l'admin crée des comptes dans Pocket ID.

**URL de rappel exacte plutôt qu'un joker** (variante plus propre) : ajouter à Looty une variable facultative, par exemple `OIDC_PROVIDER_ID` (un UUID fixé par le générateur), utilisée par `seed_default_provider` comme `id` de la ligne. Le conteneur d'amorçage enregistre alors l'URL exacte `https://<APP_DOMAIN>/api/auth/callback/<uuid>`, ce que la doc Pocket ID préfère aux jokers. Le joker `…/callback/*` marche sans toucher à Looty, mais il accepte n'importe quel identifiant de fournisseur sur ce domaine. Le risque est faible, puisque le domaine reste celui de Looty.

### 7.3 Autres pistes

| Piste | Pour | Contre |
| --- | --- | --- |
| **A. Conteneur d'amorçage + `STATIC_API_KEY`** (recommandé) | Aucune action manuelle ; pas de nouveau code dans Looty (hormis `OIDC_PROVIDER_ID`, facultatif) ; idempotent ; même logique possible en NixOS avec un `systemd.services.pocket-id-looty-init` de type `oneshot` | La clé statique est un accès admin permanent tant qu'elle reste dans `.env`. On peut la retirer après l'amorçage (l'utilisateur synthétique est alors supprimé, §1), mais ça demande une modification manuelle. Dépend de l'API ≥ v2.14 |
| **B. Looty enregistre lui-même son client** au démarrage (le backend appelle l'API avec la clé statique) | Une seule pièce ; Looty connaît son `provider_id` et peut enregistrer l'URL exacte ; erreur visible dans l'interface | Couplage de Looty à l'API d'un fournisseur précis ; la clé admin Pocket ID vit dans l'environnement de Looty, dont le rayon d'action est plus large ; code Rust à maintenir au gré des versions de Pocket ID |
| **C. Écrire directement dans la base de Pocket ID** | Pas de clé API | Schéma interne (secrets hachés, colonnes JSON) qui change d'une version à l'autre ; fragile ; à proscrire |
| **D. Guide manuel** (repli exigé par l'issue) | Toujours possible | Étapes manuelles : `/setup`, Administration → OIDC Clients → Add, saisir le client ID `looty`, la callback `https://<APP_DOMAIN>/api/auth/callback/<id>` (Looty peut afficher l'URL exacte), copier le secret dans l'administration de Looty |

### 7.4 Risques et questions ouvertes

- **`/setup` ouvert à tous** entre `up -d` et la création du compte. Les parades sont toutes côté Looty ou opérateur, Pocket ID ne propose rien : une étape « créez votre compte maintenant » dans le README généré, un `install.sh` qui affiche l'URL, ou un `basicauth` Caddy temporaire sur `/setup` (à concevoir).
- **Accès de Looty au fournisseur** : `OIDC_ISSUER=https://auth.<domaine>` doit être joignable **depuis le conteneur backend** (découverte, token, userinfo). Sans NAT en épingle sur l'hôte, ça échoue. Pistes :
  - un alias réseau Docker `${AUTH_DOMAIN}` sur le service `caddy`, pour que le backend passe par Caddy en interne avec un certificat valide ;
  - `INTERNAL_APP_URL=http://pocket-id:1411`, qui déplace token, userinfo et JWKS, mais ni la découverte ni l'`issuer` (§4).
  Pour Traefik, externe au compose, il faut une solution équivalente. **À tester de bout en bout.**
- **Version de l'API** : épingler l'image (pas seulement `v2`) et tester le script dans la CI (`deploy/test/`), puisque la route des secrets a déjà changé une fois (v2.14) et que `AUTO_CREATE_OIDC_CLIENT_SECRET` a changé de comportement (v2.14 à v2.17).
- **E-mail** : Looty exige un e-mail, alors que Pocket ID le rend facultatif à `/setup` mais l'exige par défaut sur les comptes (`REQUIRE_USER_EMAIL=true`, doc L238). Le guide doit dire de renseigner l'e-mail au moment du setup.
- **Admin** : aucune claim ne dit « admin Pocket ID » (§5). Le premier compte Looty devient admin par la règle déjà en place chez Looty. Si l'admin Pocket ID crée d'autres comptes et que l'un d'eux se connecte à Looty en premier, c'est ce compte qui devient admin. Pour une règle plus stricte, on pourrait créer un groupe Pocket ID `looty-admins` à l'amorçage et faire lire la claim `groups` par Looty : nouveau code.
- **Erreur claire dans Looty** : aujourd'hui, une découverte qui échoue donne `provider_error` au moment de la connexion. Il faudrait un message dédié (« le fournisseur intégré n'est pas joignable ou le client n'est pas enregistré, voir `docker compose logs pocket-id-init` ») et un lien vers la doc de configuration manuelle.
- **NixOS** : Looty n'a pas encore de module NixOS. Pour le modèle NixOS, il faudra une unité `oneshot` après `pocket-id.service`, avec la clé statique dans `credentials`.

## Sources

Pocket ID, code (tag v2.19.2, commit `2b2750e`) :
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/env_config.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/common/reserved.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/apikey/service.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/middleware/authenticator.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/oidc_controller.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/dto/oidc_dto.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/service/oidc_service.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/model/oidc.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/controller/well_known_controller.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/oidc/claims_service.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/service/jwt_service.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/usersignup/module.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/usersignup/service.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/usersignup/dto.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/backend/internal/cmds/one_time_access_token.go
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/docker-compose.yml
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/docker/Dockerfile-distroless
- https://github.com/pocket-id/pocket-id/blob/2b2750ed61e0016f4c15630322d15ca22143cdb6/CHANGELOG.md
- `main` (non publié) : https://github.com/pocket-id/pocket-id/blob/23d7968ccdcbe2343d2e025187f226ded9aa10ea/backend/internal/claimmappingpolicy/bootstrap.go

Pocket ID, commits, PR et issues :
- https://github.com/pocket-id/pocket-id/commit/746aa71d67299bf82685ff50696a82bebce8b409 (STATIC_API_KEY, v2.2.0) et https://github.com/pocket-id/pocket-id/pull/1229
- https://github.com/pocket-id/pocket-id/pull/864 (ID de client choisi, v1.8.0)
- https://github.com/pocket-id/pocket-id/commit/9f559788 (secret de client choisi, v2.12.0) et https://github.com/pocket-id/pocket-id/issues/1574
- https://github.com/pocket-id/pocket-id/pull/1679 (plusieurs secrets, route `/secrets`, v2.14.0)
- https://github.com/pocket-id/pocket-id/commit/d259a151 (création automatique du secret rétablie, v2.17.0)
- https://github.com/pocket-id/pocket-id/commit/931a6c2adb39fb8cdfe18487f4206f8848b3d026 (fin du TOFU sur les callbacks, v2.10.0)
- https://github.com/pocket-id/pocket-id/issues/1166

Pocket ID, documentation officielle (dépôt `pocket-id/website`, commit `ac8c2fe`, publiée sur pocket-id.org) :
- https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/api.md
- https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/configuration/environment-variables.md
- https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/setup/installation.mdx
- https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/setup/reverse-proxy.mdx
- https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/advanced/callback-url-wildcards.md
- https://github.com/pocket-id/website/blob/ac8c2fed9abc1cac9e01136e6119410d28525b50/src/content/docs/docs/setup/data-export-import.md

nixpkgs :
- https://github.com/NixOS/nixpkgs/blob/9b8845711b67c9ea6674afcf853692a09c5dd120/nixos/modules/services/security/pocket-id.nix
- https://github.com/NixOS/nixpkgs/blob/df4f7858694c1966d0b113f56164d7c855d0e42b/pkgs/by-name/po/pocket-id/package.nix

Looty (ce dépôt) : `backend/src/main.rs` (`seed_default_provider`), `backend/src/auth.rs` (`get_oidc_client`, `begin_login`, `resolve_user`), `deploy/templates/docker-compose.yml`, `deploy/templates/Caddyfile`, `deploy/render.sh`, `docs/adr/0005-install-generator-runs-in-the-browser.md`.
