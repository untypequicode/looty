# Personal items follow their owner to the personal space

A personal item (GLOSSARY: *Élément personnel*) lives in a shared workspace but is visible only to its owner. When the owner leaves the workspace, is removed from it, or the workspace is deleted, those items must not be orphaned nor exposed.

Decision: **they move to the owner's personal space**, grouped under the old workspace's name:
- documents (with their sub-documents and the projects they carry) under a page named after the workspace, created at the root of the personal space or reused if one already has that title;
- personal boards as they are (with their board calendar and its events);
- personal tasks of a shared board in a board named after the workspace (created with default columns or reused), without the old board's labels, linked groups or event links;
- non-document projects as they are;
- Drive items in a folder named after the workspace (a name already taken gets a « (2) » suffix).

Moved items stop being marked personal: the personal space is private already.

Why:
- Deleting them would lose a student's own work (their lab report, their notebook) just because they left a promo workspace.
- Leaving them in the workspace would make them unreachable for their owner and, if ever unmarked, readable by others.
- Grouping under the workspace name keeps the context (which course or project it was about) without mixing it with the rest of the personal space.

Visibility is stored on every dependent item (`private_to` copied to sub-pages, sub-tasks, tasks of a personal board, content of a personal folder), so access checks stay a single `private_to IS NULL OR private_to = me` test, admins included. Deleting the account deletes the personal items with it.
