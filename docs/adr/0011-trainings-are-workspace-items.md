# Trainings are workspace items that follow the rights of documents

Replaces the sharing and ownership parts of ADR 0009. A study set (now GLOSSARY: *Entraînement*) lived in its own `/study` section with three visibilities (`private`, `workspace`, `link`), could not be moved, and only its owner could delete it. That ignored how Looty already treats pages.

Decision: **an entraînement is a workspace item like a page**; only its content and type differ.

- **Placement**: it has a `parent_id` and appears in the workspace tree among pages. It can be moved (same dialog and rules as pages, including to another workspace), renamed, duplicated, sent to the trash and restored. It opens in a tab of the normal shell (`/w/[ws]/…`), never full screen. A list page "Entraînements" in the sidebar shows all of them with search and a type filter.
- **Personal**: `private_to` as in ADR 0002. Only its author makes it personal and only its owner re-shares; administrators do not see it. The old `private` visibility maps to personal, `workspace` to a normal item, and `link` is dropped.
- **Rights**: no new permission. The workspace role's documents level decides: read to see and take it, edit to modify, move and delete. Attempts and card state stay per user and are never shown to anyone else, the author included (ADR 0009).
- **Links**: many-to-many links to any item of the workspace (page, task, event, board, project…), shown on both sides and only to people who can see the target. Agents use them as sources.
- **Migration**: sets with workspace visibility become root items of their workspace; private sets become personal items of their owner; sets of the personal space go to its root; link tokens are revoked.

Why: one mental model, no special section, and every existing gesture (move, personal, trash, role check) works without a second implementation.

Cost: sharing by public link no longer exists for trainings; it can come back later on the model of the page link.
