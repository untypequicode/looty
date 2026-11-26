# 0022 - Page access model: audit, target model and phases

Status: phases 1, 2 and 5 shipped (audit, tests, one access resolver, levels, deny, scope, authenticated page-bound files, live guest revocation); share dialog, groups and the rest of the target model in other phases

Context: giving someone of the instance who is not a member of the space access to a page and its sub-pages goes through the page share (Share, Access). Users report it "does not seem to work well". This ADR records how access works today (audited end to end, with executable tests in `backend/src/page_access_tests.rs`), the defects found, the target model and the plan.

## 1. How access works today

- **Space role** (`workspace_members`, `workspace_roles`): `Perms` (docs, boards, calendar levels none/read/comment/validate/edit/manage, plus invite, share, admin, groups, announcements). `member_perms` is the single reader.
- **Page share** (`page_permissions`: page, user, role `viewer | editor | blocked`, `inherit` = sub-pages included, `allow_subpages`, created_by). The nearest grant on the page or an ancestor with `inherit` wins (`nearest_page_override`, mirrored in bulk by `visible_pages`; the two must stay equal).
  - Member: grant replaces the role for that page; `blocked` hides it; space admins ignore grants.
  - Non-member ("guest", `Role::Guest`): only `viewer` (read) and `editor` (write) exist; `blocked` is refused (`not_a_member`). Guests have no space rights at all.
- **Who can share**: `PUT /workspaces/{ws}/pages/{id}/permissions` and `GET` need the space `admin` right. Members with the `share` right can only create the public link (`POST /share`), and need write access to the page.
- **Public link**: `pages.share_token` + `is_public`, read-only, unauthenticated (`/api/public/pages/{token}`), including the notebook/PDF/images of the page project.
- **Enforcement points**: `page_access` / `require_page_read|write` (single page), `visible_pages` (lists, tree, search, canvases, graph, classes, activity), `require_member_or_guest` (the few space reads the UI needs: space, pages, members, prefs, notifications, calls, projects), `require_member` elsewhere (guests excluded), `ws.rs` (realtime room, guests only get filtered presence, members get events filtered per page), `collaboration/index.js` (cookie forwarded to `GET /api/pages/{id}/access`, revalidated on `NOTIFY looty_access` and every 2 minutes).
- **Shared with me**: `GET /api/shared` (roots from `page_permissions` where the person is not a member + sub-pages checked one by one with `page_access`), frontend `useShared`, `/shared` page, e-mail on first share.
- **Groups** (`member_groups`) are per space and only contain members; they carry no rights and cannot receive a page share.
- **Files and images**: uploads are `/uploads/{random-name}` served without authentication (capability URL); project files go through project routes that follow page access.
- **API tokens** authenticate as the user (same `AuthUser`, same checks). **Webhooks** are space-level configuration. **Exports**: a person's export covers their own spaces; the space export is for admins.

## 2. Findings

Fixed in this PR (tests first, all in `page_access_tests.rs`):

| # | Defect | Fix |
|---|--------|-----|
| F1 | A guest could not search at all (`search` required membership): shared pages never appeared in search | Search accepts guests, restricted to visible pages |
| F2 | Search took the 40 latest matches of the space and filtered by access afterwards: a person who sees few pages got empty or partial results when other pages matched | Visible ids are part of the query |
| F3 | `GET page` and the page list returned `share_token` (the public link) to read-only people and guests, although realtime events already hid it | Token returned only with write access |
| F4 | Moving a page inside the space changed inherited access but did not signal the collaboration server: open sessions of people who lost access lasted up to 2 minutes | `PAGE_PERMISSIONS_UPDATED` emitted on move |

Confirmed correct (covered by tests): outsider reads the shared page and its sub-pages only; no parent, sibling or space leak in lists, search paths or 404 vs 403; read guests cannot write; guest editors cannot move a page to the root or to an unshared page; page-only share (`inherit=false`) stops at the page; a grant on a sub-page overrides the parent one; revoking cuts page, collaboration check, revisions, comments, Shared with me and the space list at once; blocked members lose the page and its sub-pages.

Not fixed here, planned (larger or product decisions):

- G1. **Attachments and images are capability URLs**: `/uploads/{name}` has no authentication and no link to a page, so revoking access cannot cut a file whose URL the person already has, and a leaked URL works for anyone.
- G2. **Levels**: only read/write for guests. No comment-only, no manage. Today a read-only guest can still post comments (the comment route only checks that the page is readable), but cannot resolve threads: neither rule is a decision.
- G3. **No deny or removal for outsiders** on a sub-page of a shared tree (`blocked` is members only), and no "this page only" for a single sub-page except by granting again with `inherit=false`.
- G4. **Who may share**: only space admins manage page shares; the `share` right (members) does not allow giving a guest access; guests cannot re-share. No "manage" level on a page.
- G5. **Targets**: persons only. No group, and the people picker (`/api/users/search`) is the whole instance directory, with no restriction.
- G6. **Mentions and notifications**: `mentioned_people` keeps space members only, so guests cannot be mentioned and the editor offers no guest in mentions; sharing a page notifies by e-mail only (no in-app notification).
- G7. **Guest editors can trash the page that was shared with them** (and its whole subtree); they cannot restore it (trash needs membership).
- G8. **Realtime for guests**: the guest page set is read once at connection; guests receive presence only, so their sidebar does not update live (new sub-pages, renames) until a refetch.
- G9. **Move rules**: moving between spaces drops all page shares (documented in the code, not told to the user); moves inside a space silently change inherited access with no warning to the mover.
- G10. **Shared with me** does N+1 queries (`page_access` per descendant) and has no search, grouping or access-origin display. The share dialog shows the origin of inherited grants but not "who can see this" in one summary.
- G11. Page shares have no audit log entry and `webhooks` have no per-page filtering (space-level by design).
- G12. A suspended account keeps its `page_permissions`; checks happen at login and at the 2-minute socket tick.

## 3. Access matrix (current behavior, verified)

Page relation: S = page shared with the person, D = descendant of S (grant inherited), P = ancestor of S, X = unrelated page of the space.

| Person | Action | S | D | P | X |
|--------|--------|---|---|---|---|
| Owner / admin | read, edit, share, delete | yes | yes | yes | yes |
| Member (role docs edit) | read, edit | yes | yes | yes | yes |
| Member, `blocked` on S | read/list/search/collab | no (404) | no | yes | yes |
| Guest viewer | read, list, search (F1), shared-with-me, comments | yes | yes (if inherit) | 404 | 404 |
| Guest viewer | edit, collab write, share, trash | 403 | 403 | 404 | 404 |
| Guest editor | edit, collab write, create sub-page (with option) | yes | yes | 404 | 404 |
| Guest editor | share, change permissions | 404 (not a member) | 404 | 404 | 404 |
| Guest editor | trash S (G7), move S under a page they edit | yes | yes | 404 | 404 |
| Instance member, no share | anything | 404 | 404 | 404 | 404 |
| Anyone with public link | read page, notebook, PDF | read | no | no | no |

Surfaces: lists, tree, canvases, graph, activity, classes use `visible_pages`; search uses it since F2; breadcrumbs are built client side from the visible list (no parent title leaks); search paths stop at the first hidden ancestor.

## 4. Target model

- **Levels per page**: `read`, `comment`, `edit`, `manage` (manage = share and delete the subtree), plus explicit `deny`. Space roles keep their domain levels; a page grant can raise or lower the effective level for that page.
- **Scope**: each grant is `this page only` or `this page and sub-pages`. The nearest grant (page, then ancestors with sub-pages scope) wins; a grant or a deny on a sub-page overrides what comes from above. Deny works for members and outsiders alike.
- **Targets**: a person, several people, an instance group (new instance-level groups of accounts, plus space groups for members) or all members of the space. Invitation by name or e-mail among instance accounts, with the existing instance-directory visibility rules.
- **Visibility**: "Shared with me" (grouped by space and sharer, with access origin), search, tree and breadcrumbs limited to what the person can see, with the missing parents shown as a single "…" and no titles.
- **One resolver**: a single function `resolve_page_access(user, page) -> Level` and its bulk version `visible_pages` share one implementation and a property test (equality on random trees), used by every route, by the realtime hub and by the collaboration check. Routes that take a page id via another resource (comments, revisions, files, projects, tasks linked to a page, calls) go through it.
- **Files**: uploaded files are bound to a page (or space) and served through an authenticated route checking that access; existing capability URLs keep working for a deprecation period through a signed, expiring token form.
- **Immediate revocation**: every change of grant, deny, move, trash or membership emits `PAGE_PERMISSIONS_UPDATED`; the hub refreshes guest page sets; the collaboration server closes or downgrades connections; file tokens expire.
- **Notifications and mentions**: guests can be mentioned on pages they can read; sharing creates an in-app notification (and the e-mail); revoking is recorded in the page activity.
- **Moves**: moving inside the space shows the mover what access will be lost or gained before confirming; moving to another space keeps nothing except what the target space grants, and says so.
- **Admins and suspension**: space admins and instance admins keep access (audited); suspended accounts lose every grant effect immediately (resolver checks `is_suspended`).
- **Share dialog**: list of people and groups with level, scope and origin (direct or inherited from a named parent the person may see), search among instance accounts, one summary line ("12 people can see this page"), error pages with "request access".

## 5. Migration

- `viewer` becomes `read`, `editor` becomes `edit`, `blocked` becomes `deny`; `inherit` becomes the scope; `allow_subpages` stays an option of `edit`. No grant gains a level: `comment` and `manage` are new and only given by choice. A read-only guest who can comment today keeps commenting until their grant is edited, so no right is silently removed or added (G2 is settled by a one-time default: existing `viewer` grants get `comment`, new ones choose).
- Capability upload URLs keep working until the file migration finishes (phase 5), then only signed tokens work.
- The migration is additive (new columns, old ones read until the last phase), reversible by the feature flag, and covered by a test comparing the effective access of every (person, page) pair before and after on a generated space.

## 6. Plan

1. **Phase 1 (this PR)**: audit, ADR, matrix tests, fixes F1 to F4.
2. **Phase 2 - resolver and levels**: one access resolver, `comment` and `manage` levels, explicit `deny` for outsiders, scope per grant, backend matrix tests over every route.
3. **Phase 3 - share dialog and "Shared with me"**: new dialog (levels, scope, origin, summary), people picker, grouped Shared with me, access indicators in the tree, request-access error page, EN/FR, mobile and Android.
4. **Phase 4 - groups and notifications**: instance-level groups as share targets, guest mentions, in-app notification on share, activity log of share changes.
5. **Phase 5 - files, projects and live revocation**: authenticated and page-bound uploads, projects/notebooks/tasks in pages checked through the resolver, guest realtime refresh, collaboration downgrade on level change, trash and move rules.
6. **Phase 6 - end-to-end and docs**: e2e (outsider with page and sub-pages, revoke, inherit, override, collaboration, files), EN/FR docs and release notes.

## 7. Phase 2 (shipped in web 0.63.0): resolver, levels, deny, scope

- **One resolver** (`backend/src/access.rs`): `resolve_level(perms, grant)` is the only place that turns a person's space role and the nearest page grant (`nearest_grant`) into a level (`level::READ/COMMENT/EDIT/MANAGE`, `NONE`). `page_level` (one page: ancestors chain + grants in two queries) and `visible_levels` (a whole space) both call it; `page_access`, `require_page_*`, `visible_pages`, search, lists, tree, canvases, graph, activity, comments, revisions, projects, calls, study, AI tools, the realtime hub (`ws.rs`) and the collaboration check (`GET /api/pages/{id}/access`, now also `level` and `can_comment`) sit on top. A randomized test checks `page_level == visible_levels` on random trees, grants (all levels, deny, scopes, unknown values) and private pages.
- **Levels**: stored in `page_permissions.role` as `read | comment | edit | manage | deny`. `manage` = may list and change the shares of that page (space admins keep it everywhere); a manager can grant at most `manage`, and `PUT/GET /permissions` need it. Space role levels (docs) are unchanged: `none/read/comment/edit` map to the same levels.
- **Deny** works for members and outsiders; the nearest grant wins, so a deny on a sub-page overrides an allow above and an allow below a deny reopens a sub-tree.
- **Scope** is the existing `inherit` flag (this page only / with sub-pages); `allow_subpages` stays an option of `edit` and `manage`.
- **Compatibility**: the API still accepts `viewer` (= comment, or read for a member whose role cannot comment), `editor` (= edit), `blocked` (= deny) and returns the former name in `role` beside the new one in `level`. OpenAPI documents the permissions routes and `move-preview`.
- **Migration** (`20261215120000_page_access_levels.sql`): `editor -> edit`, `blocked -> deny`, `viewer -> comment`, except `read` for members whose role has no comment right (they could not comment). Nothing becomes `manage`. A test compares the effective rights (read, write, comment) of 35 person x page pairs before (historical rules) and after.

Gaps closed: G2 (commenting and resolving a thread follow one rule: level `comment`, for guests and members), G3 (deny for outsiders and members, per page or per tree), G4 (`manage` works for guests and members), G7 (a guest editor cannot trash the page shared with them unless they manage it; sub-pages stay trashable), G9 (moving reports who gains or loses access in `access_changes`, `GET /pages/{id}/move-preview` shows it beforehand, moving to another space returns `dropped_shares`, the permission event is emitted), G11 (`share` / `unshare` entries in the page activity), G12 (a suspended account has no level anywhere, in the single and bulk resolver).

Remaining: G1 and G5 to G6, G8, G10 and the rest of the target model (groups, instance directory limits, guest mentions, files) in phases 3 to 6; the share dialog still only shows the levels in the existing list (phase 3).

## 9. Phases 3 and 4 (shipped in web 0.68.0): share dialog, groups, request access, mentions

- **Groups** (G5): instance-level groups of accounts (`instance_groups`, managed by instance administrators). A share to a group is stored in `page_group_grants` and expanded into `page_permissions` rows (`via_group`, one per member) by `share::sync_group_grants`, so the single resolver, the realtime hub and the collaboration check need no change. A direct person share beats a group share; among groups a deny wins, otherwise the highest level. Joining or leaving a group, or deleting it, re-expands the affected pages and emits `PAGE_PERMISSIONS_UPDATED`.
- **Directory limits** (G5): `instance_directory.scope` = `all | shared | email` limits `/users/search` and group search (instance admins see everyone).
- **Who has access** (G10): `GET .../pages/{id}/access-summary` (manage level) returns people and groups with level, scope and origin (`direct | inherited | group | space`) and counts; computed in a handful of queries (`share::page_audience`, same `resolve_level`/`nearest_grant` rules). Titles of ancestors the viewer cannot see are not sent. `GET /shared` is computed per space, not per page, with level, origin, sharer and a path limited to visible ancestors.
- **Request access**: `POST /share/request-access` and `GET /share/access-status/{page}` answer the same for unknown, personal or unrelated pages (no existence leak); managers are notified (`access_request`) and approve (`read | comment | edit`, never manage) or decline from the notification or the dialog; a declined request waits one day.
- **Mentions and notifications** (G6): people outside the space who can read the page are mentionable (`.../mentionable`), notified for comments and in-page mentions; sharing creates an in-app `share` notification (events `page_shared` and `access_request` are in the notification settings; the first-share email to a guest stays).
- **UI**: new Share dialog open to anyone with manage level (`can_manage` on the page), request-access page, reworked Shared with me with realtime refresh, move preview confirmation, admin Sharing settings.

Remaining: phase 6 (end-to-end and docs).
## 8. Phase 5 (shipped in web 0.67.0): authenticated files, live revocation

- **G1 closed. Files are page-bound and authenticated.** `/uploads/{name}` keeps its URL (no document breaks) but is served by `backend/src/file_access.rs` (`decide`). Tables `upload_files` (name, uploader, space, page, task, kind: `upload | task | public | unattributed`) and `upload_refs` (pages that cite a file). The editor sends `POST /api/upload?page=<id>` (or `?workspace=<id>`; the page must be at least commentable) so the file is bound at upload time. Rules, first match wins: `LOOTY_UPLOADS_PUBLIC` compatibility switch; `public` files (avatars, favicon: only their uploader or an instance admin can make a file public); public link of a page (`?share=<token>`, the file must appear in that page; `GET /api/public/pages/{token}` tags the image URLs of its content and icon); short-lived signed link (`?sig=<exp>.<hmac>`, 10 minutes, `GET /api/uploads/{name}/link`, same access check to mint); instance admin; uploader; reader of the bound page through `page_level` (the single resolver) or of the bound task (board right) or member of the bound space; reader of any page that still cites the file (`upload_refs`, verified on each request, filled by a search when missing). Denied: `404` when signed in, `401` otherwise. Responses are `private, no-cache` with an ETag (a 304 goes through the same check, so revoking cuts a file the browser already has), `Vary: Cookie, Authorization`, and single range requests. Public files get `public, max-age=86400`.
- **Migration** (`20261216120000_upload_files.sql`): fills `upload_files` and `upload_refs` from page HTML, icon and collaborative state, task descriptions, comments and attachments, avatars and the favicon (avatar and favicon become `public`; a file used by pages is bound to its first citing page). At startup `report_unattributed` lists disk files with no row and no citation: they become `unattributed` (instance admins only), counted in the log and in the storage overview (`unattributed_files`). Nothing is deleted.
- **Other readers of `/uploads`**: sending an e-mail from the mail client refuses files the sender cannot read; exports and backups read the disk directly (unchanged). Project files, drive and notebooks already had their own authenticated routes (`/api/projects`, `/api/workspaces/{id}/drive`); project routes, the AI context and study links now all go through `project_for_pool` (a project carried by a page follows the page; before, the AI context and study links only checked space membership).
- **G8 closed.** A guest's realtime connection keeps its page set up to date (`ws.rs::guest_step`): a permission change, a new, renamed, moved or deleted sub-page is forwarded (without the public link token for readers), the client refetches its tree and Shared with me, and a guest with no access left is disconnected. In `collaboration/index.js` the revalidation applies the level (`applyAccess`): edit to read makes a live session read-only without closing it, a refusal closes it.
- **Not covered here** (see PR): trash restrictions for guests (G7 was done in phase 2), unauthenticated embedded calendar/board data (they were already behind the board and calendar rights, nothing page-based bypasses them), the Android and desktop apps' native download manager (they use the cookie jar in the WebView; use the signed link for a client without cookies).
- **Review follow-ups.** `serve_upload` opens the file first (a name that is not on disk is a plain 404, never searched). Citation searches run only for existing files, in sorted batches that skip already checked pages (up to 5 x 40 pages), limited to 120 searches per person per minute, and a full refusal is remembered 10 seconds for that person and file; found citations are stored in `upload_refs`. Item comments (page comments are bound to their page, event comments to the space), announcements (members of the space) and the personal notepad (author only; the notepad editor uploads with `?personal=1` and is never bound to the open page) are backfilled and searched. API tokens limited to spaces read only files of those spaces (public files excepted; files with no known space are refused to them). `uploader_id` of backfilled page files is the page creator (the real uploader is not recorded before this phase); it only grants that person read access. Public presentation images: the client adds `?share=<token>` to `/uploads/` image URLs of the public slides viewer, and the token is accepted when the page state (`slides_state`) cites the file. Making a file public (avatar, favicon) requires that the person can read it. Server-generated e-mails (digests, reminders, notifications, announcements) contain no `/uploads/` URL; a test guards that.
