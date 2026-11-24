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
- **Barre d’outils de texte** : pendant l’édition d’une cellule de texte, des boutons insèrent du gras, de l’italique, un lien, une image, une formule ou une liste à puces autour de la sélection.
- **Export** : le menu *Exporter* enregistre le notebook en page web (`.html`), en PDF (par la fenêtre d’impression du navigateur), en script Python (`.py`, un bloc `# %%` par cellule) ou en `.ipynb` autonome dont les images du projet sont jointes à leurs cellules. Les sorties sont celles affichées à l’écran.
- **Complétion du code** : dans une cellule de code, des suggestions apparaissent en tapant (ou avec Ctrl+Espace). Avec Pyodide elles viennent de Jedi, une fois Python démarré ; avec un serveur Jupyter, du noyau lui-même, à travers le serveur Looty.

## R, R Markdown et Quarto

- **Notebooks R** : dans la fenêtre *Moteur*, choisissez le serveur Jupyter du projet et un noyau R (IRkernel, souvent `ir`). Le nom du noyau et le `kernelspec` sont enregistrés dans le `.ipynb` : le notebook s’ouvre donc aussi dans JupyterLab. Les cellules ont la coloration syntaxique R ; la complétion vient du noyau R ; texte, data frames (tableaux HTML), graphiques PNG et SVG et erreurs s’affichent comme pour Python. Le moteur du navigateur (Pyodide) n’exécute que Python : une cellule R lancée avec lui affiche un message explicite, rien n’est exécuté comme du Python. Looty n’exécute jamais R lui-même, seul votre serveur Jupyter le fait.
- **Documents R Markdown et Quarto** : dans l’arborescence (ou le menu du projet quand le fichier est ouvert), *Ouvrir comme notebook* convertit un fichier `.Rmd` ou `.qmd` en un nouveau `.ipynb` à côté. L’en-tête YAML devient une cellule brute, le texte des cellules Markdown, les chunks ```` ```{r} ```` des cellules de code qui gardent leur en-tête exact (`echo=FALSE`, `fig.width=6`, lignes `#|` de Quarto). Les chunks d’un autre moteur (bash, sql…) deviennent des cellules brutes jamais exécutées. La conversion est sans perte : *Exporter* propose `.Rmd` et `.qmd` et le document revient tel qu’il était (les sorties ne sont pas écrites dans ces fichiers).
- **Rendu** : les exports HTML et PDF respectent `echo=FALSE`, `include=FALSE`, `results="hide"` et les lignes Quarto `#| echo: false`, et omettent l’en-tête YAML.
- **Pas encore disponible** : exécuter R dans le navigateur (WebR), et les panneaux façon RStudio (environnement, historique des graphiques, aide, console).
