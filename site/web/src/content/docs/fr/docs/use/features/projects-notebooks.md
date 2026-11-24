---
title: "Projets et notebooks"
description: "Petits sites web, LaTeX, notebooks, moteur d’exécution et serveur Jupyter du projet."
---

Un **projet** est une page qui contient des fichiers : un petit site web, un document LaTeX ou des notebooks Jupyter.

## Notebooks

- **Moteur d’exécution** : le code s’exécute soit dans votre navigateur (Pyodide, par défaut), soit sur le **serveur Jupyter** du projet. Le choix est enregistré dans le notebook et vaut pour tout le monde ; chaque personne a son propre noyau, les sorties sont partagées.
- **Dépendances du projet** : les bibliothèques sont listées dans `requirements.txt` à la racine du projet et installées par chaque noyau Pyodide à son démarrage. Une bibliothèque n’y entre que par un geste explicite (bouton *Installer*, `%pip install`, ou modification du fichier).
- **Modules du projet** : un fichier Python du projet qu’une cellule peut importer par son nom.
- **Serveur Jupyter** : une personne qui peut modifier le projet déclare un serveur externe (adresse, jeton, noyau par défaut). Seul le serveur Looty le contacte et le jeton n’est jamais montré. L’administrateur de l’instance doit autoriser les réseaux privés si besoin (`JUPYTER_ALLOW_PRIVATE`, voir [Jupyter côté hébergement](/fr/docs/host/services/)).
- **Lien public** : les visiteurs voient le notebook tel qu’enregistré, sans pouvoir l’exécuter.
