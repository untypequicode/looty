# Study sets: shared by workspace or link, attempts private to each user

> The sharing and ownership parts of this decision are replaced by [ADR 0011](0011-trainings-are-workspace-items.md) (visibility and link tokens are gone; rights follow the documents level). Attempts and card state stay private to each user as described below.

A study set (GLOSSARY: *Jeu de révision*) is authored once and taken by many people. Decision:

- **Sharing**: a set is `private` (owner only, the default), `workspace` (members of its workspace) or `link` (anyone logged in who holds an unguessable token). Sets are never listed publicly; the list only shows my sets and those shared with a workspace I belong to.
- **Roles**: the owner edits and shares. In `workspace` mode the existing document permission (`docs`) decides: read to take the set, edit to modify it. No new permission was added to `Perms`. Link holders can only take the set.
- **Answers stay on the server** for people who can only take the set: expected answers and explanations come back with the correction of an attempt (flashcard backs are part of studying and are sent).
- **Attempts and card state are per user**: every read filters on the caller's id and no route exposes another person's attempts, the owner included. Showing results to the owner would need an explicit later decision.
- **Spaced repetition** is a simplified SM-2 (ease, interval, due date per user and card).
- **Item format** is a strict JSON schema (`docs/study-items.md`) because an AI will generate sets.

Why: revision data is personal and can reveal weaknesses; a teacher sharing a QCM must not be able to read students' scores, and a leaked link must grant only taking the set.
