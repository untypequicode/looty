# 0023 - Presentations: one collaborative Yjs document per page, DOM and SVG rendering

Status: accepted

Context: issue #299 asks how Looty stores and renders slide decks (Google Slides, PowerPoint or Canva style) before the editor is built, and issue #300 asks for the editing core. Later issues (#301 to #307: themes and masters, presenter mode, import and export, creative tools, accessibility, mobile) build on this decision, so the data model is versioned from the start.

Decision:

**A presentation is a page.** A new page kind "presentation" is a page whose `default_view` is `slides`, next to `doc` and `canvas`. It reuses everything pages already have: tree, title, icon, keywords, favorites, trash, permissions and sharing (`/api/pages/{id}/access`), comments, calls, search, graph. It is created with `POST /workspaces/{id}/pages` and `default_view: "slides"`.

**One Yjs document per presentation**, named `slides:<page id>` on the collaboration server, exactly like the canvas (`canvas:<page id>`). The state lives in `pages_all.slides_state` (BYTEA), with derived columns written by the collaboration server on every store: `slides_count` (slides), `slides_text` (an outline of all text: search, graph and account export) and `slides_updated_at`. The page `content` and `yjs_state` stay empty. Access checks reuse the page ones.

**Data model** (`frontend/app/slides/model.ts`, constant `SLIDES_SCHEMA = 1`):
- `meta` (Y.Map): `schema`, `width`, `height` (canvas in logical units, default 1920x1080, 16:9).
- `slides` (Y.Map id -> slide Y.Map): `id`, `pos` (fractional order index: ordering is by `pos` then `id`, so two people reordering at once never duplicate or lose a slide), `layout` (`blank` for now, reserved for #302), `background` (color key or `#rrggbb`), `notes` (Y.Text, #301), `elements` (Y.Map id -> element Y.Map).
- element Y.Map: `id`, `type` (`text`, `rect`, `ellipse`, `line`, `image`, `group` reserved), `x`, `y`, `w`, `h`, `rotation` (degrees), `z` (order inside the slide), `locked`, `groupId`, `alt`, and `props` (Y.Map of scalars: `fill`, `stroke`, `strokeWidth`, `fontSize`, `color`, `align`, `src`...). A text element also holds a `text` Y.XmlFragment, the same ProseMirror schema subset as pages (paragraph, lists, bold, italic, underline, strike, code, link), edited by TipTap bound to that fragment.
- Every value is its own Y.Map key, so two people moving and recoloring the same element merge instead of overwriting each other; concurrent changes of the same key are last-writer-wins. Deleting an element while someone edits it: the delete wins. Z-order is a number per element, normalized to 0..n-1 by the helpers; ties are broken by id so every client renders the same stack. Groups are a shared `groupId` on members.
- Colors are palette keys (the 11 `--pal-*` of ADR 0021) or `#rrggbb`; documents never store theme-dependent CSS.

**Rendering is DOM plus SVG.** Each slide is a 1920x1080 box scaled by one CSS `transform: scale()` to fit its container (editor, thumbnails, later the presenter). Text and images are absolutely positioned DOM nodes (real text: selectable, accessible, translatable, printable); shapes are inline SVG. No canvas 2D: it would duplicate the text engine and break accessibility. Thumbnails render static HTML from the Yjs data (no editor instance) and only for slides in view.

**Media** are attachments, uploaded with the existing page attachment mechanism (`/uploads/...` URLs), referenced by `props.src`; a presentation never embeds binary data in Yjs.

**Schema versioning and migration policy.** `meta.schema` is written when the document is created. The collaboration server refuses clients announcing an older schema than `SLIDES_SCHEMA` (query parameter `schema`, `outdated-client`), the same mechanism as `DOC_SCHEMA` for pages. A change that older clients could damage (new required field, changed meaning) bumps `SLIDES_SCHEMA` in `model.ts` and `collaboration/index.js` together and ships a pure `migrate(doc)` step run on load; additive optional fields do not bump it, and unknown element types or props are preserved and rendered as an inert placeholder rather than dropped. A client older than the document becomes read-only.

**Limits** (enforced by the helpers): 300 slides, 500 elements per slide, 20 000 characters of text per element, 10 MB per image (existing upload limit).

**Undo** is per person: a Yjs UndoManager tracking only the local transaction origin (and the text editor's), scoped to the slides and meta maps.

**Presence**: awareness carries `user`, the selected element ids and the current slide, shown as colored outlines on the canvas and avatars in the slide rail.

Not decided here (later issues): themes and masters (#302; `layout` and `background` are the hooks), presenter mode and animations (`notes` exists), import and export, charts and tables.

**Collaboration (web 0.64.0).**
- *Comments* reuse the page comments (`item_comments`, kind `page`): a nullable `anchor` column names a slide (`slide:<id>`) or an element (`el:<slide>:<element>`); only a thread root carries one, only presentations accept one, and the backend validates the format. Access, mentions, resolving and notifications are the page ones (readers cannot comment, notified people must be able to read the page). A deleted slide or element leaves its threads, shown as detached.
- *Versions* reuse `page_revisions`: for a presentation (`default_view = 'slides'`) the `yjs_state` column holds the Yjs state of the slides document, recorded by the collaboration server on each store (one automatic version per editing session) and by the explicit snapshot endpoint. Restoring is done by the client inside the live document (`restoreFrom`): one LOCAL transaction that deletes the slides absent from the version and rewrites the ones that differ, leaving identical slides untouched, after an automatic "before restore" version. Everyone receives it as a normal edit and the person who restored can undo it.
- *First slide*: a brand-new presentation is initialised by the collaboration server (`initialSlidesState`) with a fixed slide id (`INITIAL_SLIDE_ID`), and clients that still initialise write the same key, so simultaneous opens converge on one slide.
- *Presence*: awareness also carries the element being edited; people editing a text box are shown live (read-only editors bound to the same fragment, with the same caret extension as pages), keyed on the fragment identity so a restoration recreates them.
- *Sections* are a `section` title on the first slide of each group (additive field, no schema bump); *arrow* and *icon* are new element types (older clients show the inert placeholder). View settings (zoom, grid) are personal.

Consequences: slides behave like pages for permissions, trash, search and links with little backend code; the document is a single unit (no per-slide lazy loading), bounded by the slide limit; PowerPoint-level fidelity (arbitrary fonts, effects) is out of scope in favor of Looty tokens and the page text schema.
