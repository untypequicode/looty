# Notebook libraries install in the browser, from the project's requirements.txt

People want to use any Python library from a notebook (`import chess` failed with a bare `ModuleNotFoundError`) and to import the project's other Python files. The obvious way to offer "any library" is an environment per project on the Looty server (a venv, a container), with a kernel Looty starts itself.

Decision: **Looty still runs no Python on its server.** With the default engine, a library is downloaded and installed by Pyodide in the browser of the person who runs the cell. The list of libraries a project needs is an ordinary project file, `requirements.txt` at the project root (see GLOSSARY: *Dépendances du projet*); every Pyodide kernel installs it when it starts. A library enters that list only through an explicit gesture: the "Installer" button shown under a `ModuleNotFoundError`, a `%pip install` line, or editing the file. A missing import is never installed silently.

Why:
- Server safety: a server-side kernel means running arbitrary code from any member on the Looty host, with isolation, quotas and a Python image to maintain. The browser sandbox gives that isolation for free, and ADR 0003 already delegates server-side execution to an external Jupyter server.
- `requirements.txt` is a file like any other: visible in the tree, shared with collaborators, exported with the project, and understood by Jupyter and pip outside Looty. No second store, no dedicated panel.
- Explicit installs only: installing whatever name appears after `import` would fetch an arbitrary PyPI package on a typo, and package code runs inside the person's Looty session.

Consequences: only pure-Python libraries and those built for WebAssembly work with the default engine; anything else needs the project's Jupyter server, and the error says so. Libraries are reinstalled at each kernel start (the browser cache softens the download). Nothing pins what "the environment" contained on a given day beyond what `requirements.txt` says. The Jupyter engine is untouched: its libraries are those of the remote server, and project files are not sent to it, so importing a project module only works with Pyodide for now.
