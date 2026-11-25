# Remote Jupyter kernels are reached through the backend

A notebook can run on an external Jupyter server instead of Pyodide (GLOSSARY: *Moteur d'exécution*, *Serveur Jupyter du projet*). The browser could talk to Jupyter directly (REST + kernel WebSocket), but that needs the token in the browser and CORS/WebSocket origin settings on every Jupyter server.

Decision: **the Looty backend relays everything** (`/api/projects/<id>/jupyter/…`, `backend/src/jupyter.rs`):
- the server address and token are stored per project, the token encrypted (`crypto.rs`) and never sent back to the interface;
- the backend adds `Authorization: token …` to REST calls and to the kernel WebSocket it opens, and relays frames to the browser (session cookie);
- the engine choice (`metadata.looty.engine`, optional `metadata.looty.kernel`) lives in the shared `.ipynb`, which therefore never contains a secret;
- one kernel per person and notebook, like Pyodide; the backend only touches kernels it started, for the person who started them, and shuts down kernels left without a connection for 20 minutes;
- seeing the configuration needs read access to the project; changing it or running code remotely needs edit access.

Why:
- Tokens stay out of URLs, browser storage, logs and shared documents.
- No CORS or WebSocket origin setup on the Jupyter side: only Looty's server contacts it.
- Errors (unreachable, token rejected, kernel gone) are mapped to explicit codes; there is no silent fallback to Pyodide.

Consequences: the Looty server must reach the Jupyter server. Like CalDAV, http and https are allowed, but private addresses (local network, localhost) only with `JUPYTER_ALLOW_PRIVATE=true`. For JupyterHub, the address may be the hub's (`/hub/login…`): the token owner is read from `/hub/api/user`, the address becomes `/user/<name>/`, and a stopped user server is started through `/hub/api/users/<name>/server`. Password and OAuth logins are not supported: users give a Jupyter token or a JupyterHub API token. Kernel ownership is kept in memory: after a backend restart, the notebook reports its kernel as gone, **Restart kernel** starts a new one, and orphaned kernels are left to Jupyter’s own culling.

PC tunnel: a project can also reach a Jupyter on a person's own computer through an outgoing connection, see ADR 0020.
