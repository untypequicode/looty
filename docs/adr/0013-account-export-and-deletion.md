# Account export and deletion

People must be able to leave an instance with their data (GDPR), and an instance must be able to forget someone without breaking the workspaces they took part in. GLOSSARY: *Archive d'export*, *Suppression de compte*, *Compte supprimé*.

Decisions:

**Export** is one zip streamed from a temporary file (never built in memory), two at a time at most.
- Account export (`GET /api/user/export`): per workspace the person belongs to, what they wrote or own: documents they authored, their personal items, tasks assigned to them or personal, events they created, Drive files and projects they uploaded or created. Their personal space is exported whole. Documents come as Markdown (from the collaborative state, falling back to the legacy HTML) and HTML; tasks as CSV and JSON; events as one `.ics`; images and attachments under `files/`, linked relatively from the documents.
- Workspace export (`GET /api/workspaces/{id}/export`, workspace admins): everything shared in the space in the same layout. Personal items of members are left out, even for admins, as everywhere else (ADR 0002). A personal space cannot be exported this way: it belongs to its owner.
- Documents are plain Markdown/HTML folders, so the existing document import (Markdown zip) reads them back on another instance. Tasks come back through the CSV/JSON files; there is no task importer for them yet.

**Deletion** is asked by the person, confirmed by retyping their email, and carried out after a grace period (`ACCOUNT_DELETION_GRACE_DAYS`, default 14, 1 to 90) by an hourly loop. Until then the account works normally and the request can be cancelled.
- Personal items and the personal space are deleted with their files (database foreign keys do the rows; the files that only they used are removed from disk). Spaces where the person is the only member are deleted too.
- Shared items stay. Documents get the technical account *Deleted user* (`00000000-0000-0000-0000-000000000000`, created at the first deletion, suspended, hidden from the administration lists and counts) as author, because `pages.created_by` cannot be null; task comments keep their text with that author; assignments and validations are removed; other authorship columns already become null.
- A person who is the only owner of a space that has other members cannot ask for deletion (nor be deleted by the loop): they must make someone else owner, or delete the space. Likewise the last administrator of the instance. Promoting someone automatically would give a space to a person who never agreed to run it, and deleting it would destroy other people's work.
- Files used by a deleted personal item inside a shared workspace may stay on disk unreferenced; the administration's storage page lists them as unused and can remove them.
- The demo does not allow deletion.

Why:
- A grace period and a typed confirmation guard against an irreversible click and a hijacked session.
- Keeping shared work with an anonymised author preserves what other people depend on (a course page, a task history) while removing the person.
