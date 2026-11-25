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

**Dépendances du projet**:
Liste des bibliothèques Python dont les notebooks d'un projet ont besoin, tenue dans le fichier `requirements.txt` à la racine du projet. Elle est partagée par tous les notebooks et toutes les personnes du projet ; chaque noyau Pyodide les installe à son démarrage. Une bibliothèque n'y entre que par un geste explicite (bouton « Installer », `%pip install`, ou modification du fichier).
_Avoid_: environnement, paquets installés, venv

**Module du projet**:
Fichier Python du projet (hors notebook) qu'une cellule peut importer par son nom, dossiers compris. Sa dernière version enregistrée est prise en compte à chaque exécution de cellule.
_Avoid_: script, librairie locale

**Lien public**:
Adresse secrète qui permet de lire une page sans compte, en lecture seule. Si la page porte un notebook, le visiteur le voit tel qu'il est enregistré (texte, code, sorties, images) sans pouvoir l'exécuter. Si elle porte un projet LaTeX, il voit le dernier PDF compilé. Il ne voit jamais les autres fichiers du projet.
_Avoid_: lien de partage, page publiée, partage public

**Document R Markdown**:
Fichier `.Rmd` ou `.qmd` (Quarto) : en-tête YAML, texte Markdown et chunks de code ```{r}. Looty le convertit en notebook (et inversement) sans perte, mais ne l'exécute pas lui-même : les cellules R tournent dans le navigateur (moteur WebR) ou sur le serveur Jupyter du projet (noyau IRkernel).
_Avoid_: rapport R, knitr

**Moteur R du navigateur (WebR)**:
Exécution des cellules R dans le navigateur de la personne qui clique, sans serveur : R compilé en WebAssembly (WebR), servi par l'application elle-même et chargé seulement au premier calcul R. Choisi par notebook dans la fenêtre Moteur (« R (navigateur) »), à côté de Pyodide (Python) et du serveur Jupyter. Les paquets viennent du dépôt WebR, seulement quand le code les installe ; `r-packages.txt` (à la racine du projet) liste ceux à installer au démarrage.
_Avoid_: R local, R embarqué

**Requête silencieuse**:
Code envoyé au noyau d'un notebook (Jupyter ou WebR) hors de toute cellule (panneaux R : variables, aide, console) : sans historique, sans numéro d'exécution, le résultat revient au panneau et n'est jamais enregistré dans le notebook.
_Avoid_: cellule cachée

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

## Pages

**Diagramme**:
Bloc d'une page qui dessine un schéma à partir d'une description écrite (organigramme, séquence, gantt…), disposé automatiquement. Son style peut suivre le style par défaut de l'espace ou en être fixé un autre.
_Avoid_: graphe (réservé au bloc graphe), schéma, Mermaid (nom de l'outil)

**Bloc graphe**:
Bloc d'une page qui montre un graphe au sens mathématique : des nœuds et des arcs, orientés ou non, portant des étiquettes (poids, flot, capacité), que l'on place à la main ou dispose automatiquement, et dont certains éléments peuvent être mis en avant (chemin, coupe). À ne pas confondre avec le graphe de l'espace, qui relie les pages, tâches et tableaux.
_Avoid_: graphe (seul), réseau, diagramme

**Style de diagramme**:
Apparence nommée d'un diagramme ou d'un bloc graphe, appliquée d'un clic : couleurs en thème clair et sombre, aspect net ou croquis, disposition, forme des nœuds, police. Le sens de lecture et les formes écrites dans la description en font partie, pas du style. Une description qui fixe elle-même son apparence garde la priorité.
_Avoid_: thème (seul), skin, mise en forme

**Style par défaut de l'espace**:
Style de diagramme que suivent tous les diagrammes et blocs graphe de l'espace qui n'en ont pas fixé un autre ; le changer les modifie tous.
_Avoid_: thème de l'espace, style global

**Dessin**:
Bloc d'une page où l'on dessine à main levée, formes et flèches libres, sans description écrite derrière.
_Avoid_: diagramme, croquis, Excalidraw (nom de l'outil)

## Assistant IA

**Connexion IA**:
Compte d'un fournisseur de modèles (OpenAI, Anthropic, Gemini, Perplexity ou serveur compatible comme Ollama) relié par une personne, avec son modèle et sa clé d'API. La clé est chiffrée sur l'instance et n'est jamais renvoyée.
_Avoid_: clé d'API (seule), intégration, compte IA

**Conversation IA**:
Discussion d'une personne avec un modèle, liée à une page, un projet, un espace ou à rien (globale). Elle lit seulement ce que cette personne peut consulter.
_Avoid_: chat, session

**Autorisation d'écriture**:
Droit donné par une personne à une conversation IA d'écrire sur une page précise, dans un mode (suggérer, ajouter ou remplacer) et pour une durée limitée ; révocable à tout moment. Sans elle, l'assistant est en lecture seule, et elle ne dépasse jamais les droits de la personne sur la page.
_Avoid_: permission, accès, droit d'écriture (réservé aux rôles)

**Autorisation pour les entraînements**:
Droit donné à une conversation IA, pour une durée limitée et révocable, soit de créer des entraînements dans un espace (ou l'espace personnel), soit de modifier un entraînement précis. Ceux créés par l'assistant sont personnels ; une modification garde la version précédente pour pouvoir l'annuler.
_Avoid_: permission, droit d'écriture

**Assistant**:
Page d'un espace où une personne retrouve toutes ses conversations IA : liste à gauche (recherche, renommage, suppression), discussion à droite. Une conversation peut être ouverte seule ou depuis une page, un projet ou un entraînement, qui lui sert alors de contexte.
_Avoid_: chat, panneau IA, bulle IA

**Pièce jointe de message**:
Élément (page, tâche, tableau, événement, document, entraînement…) que la personne joint à un message pour que l'assistant le lise avec la question. Elle ne donne aucun droit d'écriture. Aussi appelée contexte joint.
_Avoid_: fichier joint, contexte (seul)

**Fichier de l'assistant**:
Fichier déposé par une personne dans une conversation avec l'assistant (PDF, document Word, présentation, texte…). Il est rangé dans le Drive personnel, dossier « Assistant IA », privé, et disparaît comme tout fichier du Drive à l'échéance de celui-ci ; l'assistant n'en lit que le texte.
_Avoid_: upload, document IA

**Texte extrait**:
Texte qu'on tire d'un fichier (Drive, Moodle ou déposé dans le chat) pour que l'assistant puisse le lire. Il vit avec le fichier et disparaît avec lui. Un fichier sans texte (PDF scanné, image) n'en a pas et n'est pas lisible par l'assistant.
_Avoid_: OCR, contenu du fichier

**Cours Moodle joint**:
Cours Moodle (ou l'un de ses fichiers) qu'une personne joint à un message pour interroger l'assistant. Il est lu avec le compte Moodle de cette personne seulement : un cours que ce compte ne peut pas ouvrir est refusé. Un cours arrive sous forme de plan (sections, activités, fichiers avec type et taille) ; le contenu d'un fichier n'est lu que quand la question le demande.
_Avoid_: import Moodle, synchronisation du cours

**Envoyer à l'IA**:
Action d'un cours ou d'un fichier Moodle qui l'ajoute au message en cours de rédaction dans l'Assistant, sans toucher au texte déjà tapé, et ouvre l'Assistant s'il ne l'est pas.
_Avoid_: partager avec l'IA, exporter

**Carte d'autorisation**:
Carte affichée dans la conversation quand l'assistant a besoin d'un droit d'écriture qu'il n'a pas : elle indique la page ou l'entraînement, le mode et la durée, et la personne l'accepte ou la refuse d'un clic. Accepter crée l'autorisation correspondante.
_Avoid_: popup de permission, demande d'accès

**Suggestion**:
Modification proposée par l'assistant en mode « suggérer » : une différence que la personne accepte ou refuse, rien n'étant écrit avant.
_Avoid_: brouillon, patch

## Entraînements

**Entraînement**:
Élément de l'espace qui sert à s'exercer ou à réviser : QCM, questionnaire, fiches ou synthèse. Il se range dans l'arbre comme une page, se déplace, se renomme, se duplique, passe par la corbeille et peut être personnel. Ses droits suivent ceux des documents de l'espace : lecture pour s'entraîner, édition pour modifier et supprimer. Il peut être lié à tout autre élément de l'espace.
_Avoid_: jeu de révision, révision (réservé à l'historique d'une page), quiz, test, cours

**Lien d'entraînement**:
Référence d'un entraînement vers un élément de l'espace (page, tâche, événement, tableau, projet…), affichée des deux côtés mais seulement à qui voit la cible.
_Avoid_: pièce jointe, source

**Tentative**:
Passage d'un entraînement par une personne, avec sa note et ses réponses. Visible de cette personne seulement, jamais de l'auteur de l'entraînement.
_Avoid_: copie, résultat partagé

**État de fiche**:
Facilité et prochaine échéance d'une fiche pour une personne (répétition espacée simplifiée).

**Formule**:
Expression mathématique écrite en LaTeX dans une page (dans le texte ou en bloc), rendue comme sur le web quel que soit l'appareil, hors-ligne compris.
_Avoid_: équation, maths

## Données

**Archive d'export**:
Fichier zip qui contient, par espace, les documents d'une personne (Markdown et HTML), ses tâches (CSV et JSON), ses événements (fichier d'agenda), ses fichiers et ses éléments personnels. Un administrateur d'espace peut aussi exporter tout ce qui est partagé dans l'espace, sans les éléments personnels des membres.
_Avoid_: sauvegarde (réservé à la sauvegarde de toute l'instance), dump

**Suppression de compte**:
Demande faite par la personne, confirmée par son adresse e-mail, exécutée après un délai de grâce pendant lequel elle peut l'annuler. Ses éléments personnels et son espace perso disparaissent ; ce qu'elle a partagé reste dans les espaces. Impossible tant qu'elle est la seule propriétaire d'un espace qui a d'autres membres.
_Avoid_: désinscription, désactivation (la suspension est autre chose : un administrateur bloque la connexion)

**Compte supprimé**:
Compte technique (« Deleted user ») qui apparaît comme auteur des éléments partagés d'une personne dont le compte a été supprimé. On ne peut pas s'y connecter et il n'est pas compté parmi les comptes de l'instance.

**État de l'instance**:
Page d'administration qui montre l'espace utilisé (base, fichiers, par espace), les comptes, espaces et sessions, les tâches de fond (dernier passage, erreurs), la dernière sauvegarde et la version. Le même état sort sur `/metrics` pour Prometheus quand un jeton est défini.
_Avoid_: monitoring (le *Diagnostic* vérifie l'installation, c'est autre chose)

**Rapport d'import**:
Liste, affichée à la fin d'un import depuis Notion, Obsidian, Trello ou Outline, de ce qui n'a pas pu être repris tel quel (cartes archivées, liens vers des pages absentes, images introuvables, membres sans correspondance).

## Intégrations

**Jeton d'accès**:
Clé personnelle (`looty_…`) qui permet à un script, n8n ou un bot d'agir au nom d'une personne par l'API, sans jamais avoir plus de droits qu'elle. Elle peut être en lecture seule, limitée à certains espaces, et expirer. Distinct du mot de passe d'application (CalDAV) et du lien d'abonnement à un calendrier.
_Avoid_: clé d'API, token (seul)

**Webhook**:
Adresse qui reçoit, pour un espace, un message signé (HMAC) à chaque événement choisi (tâche créée, modifiée ou terminée ; document modifié ; événement créé ; annonce publiée). Géré par les administrateurs de l'espace ; les éléments personnels n'y sont jamais envoyés. Chaque livraison est journalisée et réessayée en cas d'échec.
_Avoid_: callback, notification (réservé aux notifications de la boîte de réception)

**Modèle**:
Tableau (colonnes, étiquettes, tâches types) ou document enregistré par un administrateur d'un espace, proposé aux membres qui peuvent créer. Looty en fournit quelques-uns (suivi de projet, rétrospective, compte rendu de réunion, notes de cours, compte rendu de TP). Un élément personnel ne devient jamais un modèle.
_Avoid_: gabarit, template (en français)

## Installation

**Compte local**:
Compte avec adresse e-mail et mot de passe, géré par Looty lui-même, à activer avec `LOOTY_LOCAL_ACCOUNTS`. Déconseillé : on préfère OpenID Connect. L'inscription est fermée (sur invitation d'un administrateur) sauf choix contraire, et le premier compte d'une instance vide devient administrateur grâce à un code lu dans les journaux.
_Avoid_: compte interne, compte Looty (tous les comptes sont des comptes Looty)

**Double authentification**:
Code à 6 chiffres d'une application d'authentification (TOTP), demandé après le mot de passe d'un compte local, avec des codes de secours à usage unique. Obligatoire pour les administrateurs ; un administrateur peut l'imposer à toute l'instance ou la retirer à quelqu'un qui a perdu son appareil.
_Avoid_: 2FA, MFA, authentification forte

**Assistant de premier lancement**:
Suite d'étapes (connexion, nom et langue de l'instance, e-mails, notifications push, premier espace et invitations) proposée à l'administrateur d'une instance neuve, une seule fois. Chaque étape se refait ensuite depuis l'administration.
_Avoid_: installateur, onboarding (réservé à l'accueil des membres)

**Diagnostic**:
Page d'administration et commande `looty doctor` qui vérifient l'installation : adresse, HTTPS, proxy, WebSocket, CalDAV, connexion, e-mails, base et fichiers. Chaque contrôle donne OK, avertissement ou erreur avec un lien vers le dépannage. À ne pas confondre avec l'*État de l'instance* (ce qui tourne et ce que ça occupe).

## Appels

**Appel**:
Conversation vocale, avec caméra et partage d'écran en option, entre les personnes qui travaillent sur un même élément. Elle passe directement d'un navigateur à l'autre : le serveur de Looty n'écoute rien et ne garde rien. L'appel continue quand on change de page, grâce à la barre d'appel.
_Avoid_: conférence, visio (réservé à la vidéo), chat vocal

**Salle d'appel**:
Lieu d'un appel, lié à une page, un projet ou un espace. Qui peut ouvrir l'élément peut rejoindre sa salle ; on voit qui s'y trouve avant de la rejoindre. Limitée en nombre de participants (6 par défaut).
_Avoid_: canal, réunion

**Participant**:
Personne qui a rejoint une salle d'appel (distincte de celle qui regarde seulement qui s'y trouve). Un participant peut couper son micro et régler le volume de chacun des autres.
_Avoid_: invité (qui désigne un invité d'espace), interlocuteur

**Signalisation**:
Échange de petits messages (arrivée, départ, offres et réponses WebRTC, candidats réseau, micro coupé) par lequel les navigateurs se mettent en relation. C'est la seule chose que le serveur relaie dans un appel ; elle passe par le serveur de collaboration. Voir ADR 0019.
_Avoid_: serveur d'appel, relais audio

**Partage d'écran**:
Flux vidéo dédié d'un appel (écran, fenêtre ou onglet, avec le son du système si le navigateur le permet), affiché en grand chez tous les participants avec le nom de la personne qui partage. Il privilégie la netteté du texte (peu d'images par seconde) ; la caméra, elle, est limitée à 4 personnes par appel.
_Avoid_: diffusion, présentation

**Qualité de liaison**:
Mesure continue (latence, perte de paquets, gigue, débit, par `getStats`) de la connexion directe avec chaque participant, résumée par un indicateur bon / moyen / mauvais. Le débit vidéo s'y adapte ; l'audio reste prioritaire. En cas de coupure, la liaison redémarre seule (*ICE restart*).
_Avoid_: ping, signal

## Tunnel PC

**Tunnel**:
Connexion sortante qu'une personne ouvre depuis son PC vers une instance pour que celle-ci puisse utiliser ce PC (comme serveur Jupyter, de compilation LaTeX…) tant que la connexion dure. Aucun port à ouvrir ; elle se coupe quand l'agent s'arrête, et les projets reviennent alors à leur moteur par défaut. Voir ADR 0020.
_Avoid_: VPN, accès à distance, pont

**Agent**:
Petit programme (`looty-bridge`) lancé sur le PC, qui ouvre le tunnel et ne joint que les services qu'on lui a déclarés. Il s'authentifie avec un jeton de tunnel.
_Avoid_: client, démon, connecteur

**Service tunnelé**:
Service local du PC (serveur Jupyter, compilation LaTeX, service HTTP) que l'agent déclare à l'instance sous un nom et un type. L'instance ne peut joindre que les services déclarés, jamais un autre port du PC. Un projet cible un service d'un tunnel au lieu d'une adresse.
_Avoid_: port, application locale

**Jeton de tunnel**:
Secret qui authentifie un agent. Lié à une personne et à un espace (éventuellement un projet), à durée limitée, révocable, montré une seule fois. Il ne donne aucun accès à l'API de Looty, à ne pas confondre avec un *Jeton d'accès*.
_Avoid_: clé, mot de passe d'agent
