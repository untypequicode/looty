# Looty

Looty est une application de notes, documents et projets, disponible en web, bureau et Android. Ce glossaire fixe le vocabulaire partagé du produit.

## Distribution Nix

**Cache binaire**:
Le dépôt Cachix `looty` (`looty.cachix.org`) qui contient les paquets et environnements Nix de Looty déjà compilés, pour que les utilisateurs Nix téléchargent au lieu de compiler.
_Avoid_: cache Nix, cache NixOS, substituer

**Épingle**:
Nom persistant donné à un chemin du cache binaire pour le protéger de l'éviction. Les épingles de `main` gardent les 3 dernières révisions ; celles des releases desktop sont conservées sans limite.
_Avoid_: pin, verrou

**Vérification du cache**:
Contrôle qui construit le paquet et les entrées des shells du flake en n'autorisant que le cache binaire comme source, et échoue si quoi que ce soit doit être compilé. Elle tourne après chaque push sur `main`, chaque release desktop et chaque semaine.
_Avoid_: test du cache, smoke test

## Espaces

**Groupe**:
Ensemble nommé de membres d'un espace, sans aucun droit, qui sert à mentionner plusieurs personnes d'un coup ou à les ajouter ensemble à une tâche ou un événement. Une personne peut appartenir à plusieurs groupes. Ajouter un groupe à une tâche y ajoute ses membres du moment ; la tâche ne suit pas les changements ultérieurs du groupe.
_Avoid_: équipe, rôle, liste

## Moodle

**Connexion automatique**:
Ouverture du site Moodle déjà connecté avec le compte relié à Looty, sans ressaisir ses identifiants.
_Avoid_: SSO, autologin
