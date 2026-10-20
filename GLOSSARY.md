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

**Cours Moodle**:
Un cours du site Moodle auquel le membre est inscrit. À ne pas confondre avec une séance de cours du calendrier.
_Avoid_: classe, matière, module

**Tableau de bord Moodle**:
Écran d'accueil de Moodle dans Looty, composé de panneaux que chaque membre dispose librement sur une grille. Il ne s'agit pas d'un tableau de tâches.
_Avoid_: board, aperçu, dashboard

**Panneau**:
Bloc du tableau de bord Moodle (échéances, cours, dossier, cours épinglé…) que le membre place, redimensionne et règle à son goût.
_Avoid_: widget, carte, bloc

**Cours favori**:
Cours Moodle marqué d'une étoile ; la marque est la même dans Looty et sur le site Moodle.
_Avoid_: favori (seul), épinglé

**Cours masqué**:
Cours Moodle que le membre a retiré de ses listes, de la même manière dans Looty et sur le site Moodle.
_Avoid_: archivé, caché

**Catégorie Moodle**:
Classement des cours défini par l'établissement sur le site Moodle ; le membre ne peut pas la modifier.
_Avoid_: dossier, filière

**Dossier de cours**:
Regroupement de cours Moodle créé par le membre dans Looty, qui n'existe pas sur le site Moodle.
_Avoid_: catégorie, collection, groupe

**Personnalisation d'un cours**:
Nom court, couleur et icône qu'un membre donne à un cours Moodle ; elle remplace la présentation du site partout dans Looty, pour ce membre seulement.
_Avoid_: alias, surnom, thème
