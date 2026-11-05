# Looty

Looty est une application de notes, documents et projets, disponible en web, bureau et Android. Ce glossaire fixe le vocabulaire partagé du produit.

## Instances

**Instance**:
Une installation de Looty à une adresse donnée (`looty.mon-asso.fr`), avec ses comptes et ses espaces. Looty ne propose pas d'hébergement : chaque instance est auto-hébergée par quelqu'un.
_Avoid_: serveur (seul), hébergement, tenant

**Instance de l'app**:
L'instance à laquelle une app Android ou bureau est connectée ; un lien d'une autre instance ne s'ouvre pas dans l'app.
_Avoid_: serveur connecté, compte

**Démo**:
L'instance publique `demo.looty.untypequicode.fr`, remplie de données fictives et remise à zéro régulièrement, où l'on entre sans compte en choisissant un personnage. Chaque visiteur a sa propre copie de l'espace « Looty Démo » ; les fonctions qui chargent le serveur ou sortent vers l'extérieur y sont marquées « Indisponible dans la démo ». Ce n'est pas un hébergement : rien n'y est conservé.
_Avoid_: bac à sable, essai, instance de test

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
Ensemble nommé de membres d'un espace, sans aucun droit, qui sert à mentionner plusieurs personnes d'un coup, à les assigner ensemble à une tâche ou un événement, ou à leur adresser une annonce. Une personne peut appartenir à plusieurs groupes.

**Groupe relié**:
Groupe ajouté à une tâche, un événement ou une annonce, qui suit ensuite ses changements : un membre ajouté au groupe y est assigné (ou voit l'annonce), un membre retiré en est retiré, sauf s'il y était aussi individuellement ou par un autre groupe relié. Les tâches terminées ne bougent plus ; supprimer le groupe laisse les personnes en place.
_Avoid_: équipe, rôle, liste

**Annonce**:
Information qu'une personne autorisée (droit « Publier des annonces ») passe à tout ou partie des membres d'un espace (des groupes reliés et des personnes choisies) : changement de salle, date d'un partiel… Elle est notifiée à ses destinataires, chacun la voit lue ou non lue, elle peut être épinglée et expirer.
_Avoid_: message, actualité, post, news

**Sondage**:
Question à options portée par une annonce : chaque membre vote (choix unique ou multiple), change ou retire son vote tant qu'il est ouvert. Il peut être anonyme (seuls les totaux sont visibles), ne montrer les résultats qu'après le vote, et se clore à une date ou à la main.
_Avoid_: vote, questionnaire, formulaire

**Élément personnel**:
Document, projet, fichier du Drive, tableau ou tâche d'un espace partagé visible de son seul propriétaire, même pas des administrateurs : son TP sous la page du cours, son notebook dans le projet du groupe. Ce qui est rangé dedans l'est aussi (sous-pages, sous-tâches, tâches d'un tableau personnel). Seule son autrice le rend personnel, seule sa propriétaire le repartage. S'il quitte l'espace, il part dans son espace perso, sous le nom de l'ancien espace. À ne pas confondre avec l'espace perso.
_Avoid_: privé, élément privé, brouillon

## Recherche

**Mot-clé**:
Terme libre posé sur un document, un projet, un fichier du Drive, un tableau, une tâche ou un événement pour le retrouver par la recherche, même s'il n'apparaît pas dans son contenu (code du cours, « partiel »…). Il est transverse à l'espace, sans couleur ni droit ; `#mot-clé` dans la recherche ne montre que les éléments qui le portent.
_Avoid_: tag, étiquette (réservé aux étiquettes colorées d'un tableau)

## Agenda externe

**Flux d'abonnement**:
Adresse secrète en lecture seule d'un calendrier Looty, que le membre colle dans une autre application d'agenda (Infomaniak, Google, Apple, Outlook…) pour y voir ses événements et échéances. Il se révoque sans toucher au reste du compte.
_Avoid_: export, lien ICS, lien de partage

**Bouton d'abonnement**:
Bouton de Looty qui ouvre l'abonnement à un flux d'abonnement dans une application d'agenda donnée, ou en copie l'adresse.
_Avoid_: raccourci, lien rapide

**Mot de passe d'application**:
Identifiant généré par Looty pour qu'un appareil ou une application d'agenda lise et modifie les tâches et événements via CalDAV. Il ne donne accès à rien d'autre dans Looty et se révoque appareil par appareil.
_Avoid_: jeton, clé d'API

**Connexion à l'agenda du téléphone**:
Configuration, depuis l'app Android de Looty, qui fait apparaître les calendriers de Looty dans l'application Agenda du téléphone, en lecture et écriture, grâce à un mot de passe d'application.
_Avoid_: compte Android, synchro native

## Notebooks

**Moteur d'exécution**:
Où s'exécute le code d'un notebook : dans le navigateur de la personne qui clique (Pyodide, par défaut) ou sur le serveur Jupyter du projet. Le choix est enregistré dans le notebook et vaut pour tout le monde ; chaque personne a son propre noyau, les sorties sont partagées.
_Avoid_: runtime, backend

**Lien public**:
Adresse secrète qui permet de lire une page sans compte, en lecture seule. Si la page porte un notebook, le visiteur le voit tel qu'il est enregistré (texte, code, sorties, images) sans pouvoir l'exécuter. Si elle porte un projet LaTeX, il voit le dernier PDF compilé. Il ne voit jamais les autres fichiers du projet.
_Avoid_: lien de partage, page publiée, partage public

**Serveur Jupyter du projet**:
Serveur Jupyter externe (Jupyter Server, JupyterLab, serveur utilisateur JupyterHub) déclaré dans un projet par une personne qui peut le modifier : adresse, jeton, noyau par défaut. Seul le serveur Looty le contacte ; le jeton n'est jamais montré.
_Avoid_: kernel distant, Jupyter Hub (quand on parle de la configuration du projet)

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

**Note Moodle**:
Note publiée dans le carnet de notes Moodle d'un cours : la note globale du cours, ou celle d'un élément (devoir, test…) avec son barème et le commentaire de l'enseignant. Looty la lit, sans pouvoir la modifier.
_Avoid_: résultat, évaluation, score

**Personnalisation d'un cours**:
Nom court, couleur et icône qu'un membre donne à un cours Moodle ; elle remplace la présentation du site partout dans Looty, pour ce membre seulement.
_Avoid_: alias, surnom, thème

## Navigation

**Onglet**:
Page ouverte dans la barre d'onglets d'un espace, qui garde son propre chemin même quand on en change. Naviguer dans l'onglet actif change sa page, comme dans un navigateur.
_Avoid_: fenêtre, page ouverte

**Panneau de tâche**:
Volet latéral qui montre le détail d'une tâche. Il s'ouvre à droite du tableau, ou à droite de la liste dans « Mes tâches ». À ne pas confondre avec un panneau du tableau de bord Moodle.
_Avoid_: fiche, popup, tiroir

## Tâches

**Tâche récurrente**:
Tâche avec une répétition (chaque jour, semaine, deux semaines, mois ou année). Une fois terminée, elle crée sa prochaine occurrence ; elle-même reste terminée.
_Avoid_: tâche répétée, routine, modèle

**Occurrence suivante**:
Copie d'une tâche récurrente créée à sa fin : même tableau, contenu, étiquettes et personnes, dates décalées d'autant de périodes qu'il faut pour tomber dans le futur. Une seule par tâche.
_Avoid_: clone, duplicata
