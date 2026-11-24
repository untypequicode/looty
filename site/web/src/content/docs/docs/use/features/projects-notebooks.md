---
title: "Projects and notebooks"
description: "Small web sites, LaTeX, notebooks, the execution engine and the project Jupyter server."
---

A **project** is a page that holds files: a small web site, a LaTeX document or Jupyter notebooks.

## Notebooks

- **Execution engine**: code runs either in your browser (Pyodide, the default) or on the project's **Jupyter server**. The choice is saved in the notebook and applies to everyone; each person has their own kernel, outputs are shared.
- **Project dependencies**: libraries are listed in `requirements.txt` at the root of the project and installed by each Pyodide kernel when it starts. A library only enters the list through an explicit action (*Install* button, `%pip install`, or editing the file).
- **Project modules**: a Python file of the project that cells can import by name.
- **Jupyter server**: a person who can edit the project declares an external server (address, token, default kernel). Only the Looty server contacts it and the token is never shown. The instance administrator must allow private networks if needed (`JUPYTER_ALLOW_PRIVATE`, see [Jupyter for hosts](/docs/host/services/)).
- **Public link**: visitors see the notebook as saved, without running it.
- **Text toolbar**: while a text cell is being edited, buttons insert bold, italic, a link, an image, a formula or a bulleted list around the selection.
- **Export**: the *Export* menu saves the notebook as a web page (`.html`), a PDF (through the browser's print dialog), a Python script (`.py`, one `# %%` block per cell) or a standalone `.ipynb` in which the project's images are attached to their cells. Outputs are those shown on screen.
- **Code completion**: in a code cell, suggestions appear while typing (or with Ctrl+Space). With Pyodide they come from Jedi, once Python has started; with a Jupyter server, from the kernel itself, through the Looty server.
