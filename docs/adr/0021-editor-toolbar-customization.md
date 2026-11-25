# 0021 - Customizable editor toolbar, one tool model, palette highlight

Status: accepted

Context: the page editor toolbar (web 0.61.0) was a single fixed bar. People asked for a simpler color picker, a bar that adapts to its width, a top or bottom position, per-user customization and animation options, and a Word-like layout.

Decision:
- **One tool model** (`frontend/app/editor/toolbarModel.ts`): tools, groups, ribbon tabs, priorities, the fold into the More menu, the preference cleaning and the typing phase are pure functions. The compact bar and the ribbon are two renderings of the same groups, so there is no second list of buttons to keep in sync.
- **Preference `editorToolbarCfg`** in `users.prefs` (merged key by key by `PATCH /api/user/prefs`, no migration). The frontend reads it with `cleanConfig` (unknown or legacy values fall back to defaults, groups added later are appended) and writes only the differences from the defaults. `editorToolbar` (show or hide) is unchanged.
- **Palette**: text color and highlight share 11 theme keys, resolved by the `--pal-*` CSS variables in `tokens.css` (4.5:1 contrast on the page background in both themes, checked by a unit test). Documents store keys, never raw colors.
- **Highlight** gets a `color` attribute (`mark[data-highlight]`); a highlight without it keeps the former yellow, so existing documents are unchanged. Because older clients would drop the attribute and the new palette keys, `DOC_SCHEMA` goes from 4 to 5 (frontend `source.ts` and `collaboration/index.js` together): tabs opened before must reload to edit.
- **Typing options** only use opacity and transform on the bar and a single passive `keydown` listener on the editor, so typing and collaboration cursors are unaffected. Animations default to off when the system asks for reduced motion.

Consequences: customization is per account and follows the person across devices; recent colors stay on the device. The ribbon is not offered on phones, where the compact bar is docked above the keyboard.
