# Images pasted into a notebook are project files, not cell attachments

Pasting or dropping an image into a Markdown cell has to store it somewhere. Jupyter's own answer is the cell **attachment**: the image is base64-encoded inside the `.ipynb` (`attachments` field) and referenced as `![](attachment:image.png)`. That keeps the notebook self-contained, but in Looty the notebook is a shared Yjs document (`file:<id>`, see `collaboration/index.js`) synced live to everyone who has it open, and every pasted screenshot would be pushed to every collaborator and kept in the document history.

Decision: **a pasted or dropped image is uploaded as a file of the project**, in an `images/` folder next to the notebook (dropped files keep their name, pasted ones are named `collage-YYYYMMDD-HHMMSS.png`, collisions get `-2`, `-3`…), and the cell receives a relative link `![nom](images/….png)`. Relative paths in Markdown cells resolve against the notebook's folder, then the project root, like in Jupyter.

Existing attachments are still **read and kept**: notebooks imported from JupyterLab keep their `attachments` through the Yjs round-trip and `attachment:` images render. Looty just never creates new ones.

Why:
- The live document stays small: collaborators sync a path, not megabytes of base64.
- An image is an ordinary project file: visible in the tree, renamable, deletable, usable from code (`open("images/…")`).
- The public link only has to expose the project's image files by path (see GLOSSARY: *Lien public*), not parse notebooks.

Consequences: a `.ipynb` downloaded on its own loses its pasted images (they live in the project, not in the file); exporting the whole project keeps them. Deleting an image file that a cell still references leaves a broken image: there is no reference tracking. Switching to attachments later would mean rewriting every notebook that already links to `images/`.
