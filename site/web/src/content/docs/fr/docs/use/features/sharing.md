---
title: "Partage"
description: "Partager une page avec des personnes de l’espace ou extérieures : niveaux, portée, groupes, invités et demandes d’accès."
---

Ouvrez une page et appuyez sur **Partager**. Toute personne qui peut **gérer** la page peut utiliser le dialogue, pas seulement les administrateurs de l’espace.

## Qui a accès

Le dialogue commence par un résumé, par exemple « 12 personnes ont accès, dont 3 hors de l’espace », puis liste chaque personne avec :

- son **niveau** : *Peut voir*, *Peut commenter*, *Peut modifier*, *Peut gérer*, ou *Aucun accès* (un refus qui l’emporte sur le rôle dans l’espace) ;
- la **portée** : *cette page seulement* ou *cette page et ses sous-pages*. Le partage le plus proche gagne : un partage ou un refus sur une sous-page remplace ce qui vient du dessus ;
- l’**origine** du droit : ajouté sur cette page, depuis une page parente (nommée seulement si vous pouvez la voir), par un groupe, ou par le rôle de la personne dans l’espace.

Changer le niveau d’une ligne héritée, ou d’un membre qui suit son rôle, crée un partage direct sur cette page. Retirer un partage direct revient au rôle, à la page parente ou au groupe. Les changements s’appliquent immédiatement, y compris aux sessions ouvertes.

## Ajouter des personnes et des groupes

Saisissez un nom ou un e-mail. Vous trouvez les membres de l’espace, les comptes de l’instance et les groupes. Donner la **gestion** demande une confirmation : cette personne pourra partager la page avec n’importe qui et la supprimer avec ses sous-pages.

Un administrateur de l’instance peut limiter qui peut être trouvé (Réglages, Partage) : tout le monde, les personnes qui partagent un espace avec vous, ou seulement une adresse e-mail complète. Les groupes de comptes se créent aussi là ; partager avec un groupe donne l’accès à chaque membre et suit les arrivées et départs. Un partage direct à une personne l’emporte toujours sur celui d’un groupe, et le refus d’un groupe l’emporte.

## Personnes hors de l’espace

Une personne sans rôle dans l’espace qui reçoit un partage est un **invité** : elle ne voit que la page partagée et ses sous-pages (jamais les parents), dans *Partagé avec moi*. Elle est marquée *Hors de l’espace* dans le dialogue, et peut être mentionnée dans la page et ses commentaires dès qu’elle peut la lire. Un partage envoie une notification dans l’application, et un e-mail la première fois pour un invité (voir les réglages de notification).

## Demander l’accès

Ouvrir le lien d’une page qu’on ne voit pas propose **Demander l’accès**. Les personnes qui gèrent la page reçoivent une notification avec *Accepter* et *Refuser*, aussi disponibles dans le dialogue de partage. Accepter donne *Peut voir* sur la page et ses sous-pages. La page n’est jamais nommée à la personne qui demande, et une demande refusée peut être refaite le lendemain.

## Partagé avec moi

Les pages que d’autres vous ont partagées dans des espaces dont vous n’êtes pas membre sont groupées par espace, les plus récentes d’abord, avec une recherche. Chacune montre votre niveau, qui l’a partagée (ou le groupe) et un chemin limité aux parents que vous voyez, avec « … » quand d’autres sont masqués. La liste se met à jour toute seule.

## Déplacer une page

Déplacer une page sous un autre parent liste qui gagnerait ou perdrait l’accès et demande confirmation. La déplacer vers un autre espace retire ses partages individuels.

## Fichiers et images

Les images et pièces jointes suivent la page à laquelle elles appartiennent : une personne ne peut ouvrir un fichier que si elle peut lire cette page (ou cette tâche, ou cet espace). Retirer l’accès à quelqu’un arrête aussitôt le chargement des images pour lui, même s’il a gardé l’adresse. Les anciens liens continuent de fonctionner pour les personnes qui ont encore accès ; ils ne fonctionnent plus pour les autres, ni sans connexion (une personne non connectée reçoit *401*, une personne connectée sans accès reçoit *404*). Pour montrer une image à quelqu’un qui n’est pas connecté, utilisez un lien public.

## Liens publics

**Copier le lien public** dans le dialogue rend la page lisible par toute personne qui a le lien, sans compte, en lecture seule. Seule cette page est exposée (ni son parent ni ses sous-pages), avec ses propres images. Désactivez le lien : la page et ses images ne se chargent plus. Un lien public ne donne pas accès à l’espace : invités et membres gardent les niveaux ci-dessus.

## API et jetons

Un jeton d’API agit comme la personne qui l’a créé : il a exactement son accès aux pages et aux fichiers, et un jeton limité à certains espaces ne peut pas lire les fichiers des autres espaces. Les changements de partage par l’API suivent la même règle que le dialogue (il faut pouvoir gérer la page).
