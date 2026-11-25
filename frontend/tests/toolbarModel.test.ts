// node --test frontend/tests/toolbarModel.test.ts
// Barre d'outils de l'éditeur : repli en « Plus », état de la sélection, menu « Insérer », préférence, clavier.
import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import {
  TOOLBAR_COVERED, MORE_WIDTH, EMPTY_STATE, DEFAULT_CONFIG, DEFAULT_ORDER, ALL_GROUPS, ALL_TOOLS, TAB_GROUPS, MIN_OPACITY, MAX_OPACITY,
  SEP_WIDTH_TOUCH, groupWidth, splitGroups, toolbarGroups, groupTools, buttonWidth, insertItems, toolbarState, sameState, toolbarVisible, rovingNext, keyboardInset,
  cleanConfig, serializeConfig, moveGroup, setToolHidden, setGroupHidden, groupHidden, effectiveStyle, effectivePosition, animateOn, barPhase,
  pushRecent, ribbonTabs, ribbonTabOr, type ToolbarSource, type ToolbarGroupId,
} from '../app/editor/toolbarModel.ts'
import { cleanColor, cleanAlign, cleanHighlight, TEXT_COLORS } from '../app/editor/textStyle.ts'

const CFG = DEFAULT_CONFIG
const groupsOf = (cfg = CFG, inTable = false, only?: ToolbarGroupId[]) => toolbarGroups(cfg, inTable, only)
const total = groupsOf().reduce((n, g) => n + groupWidth(g), 0)

test('large screen: every group stays in the bar, no More menu', () => {
  const r = splitGroups(total + 10, groupsOf())
  assert.equal(r.overflow.length, 0)
  assert.deepEqual(r.visible, groupsOf().map((g) => g.id))
})

test('narrow screen: lowest-priority groups fold first, most useful ones stay, order is kept', () => {
  const r = splitGroups(total - 1, groupsOf())
  assert.ok(r.overflow.includes('align') && !r.overflow.includes('marks') && !r.overflow.includes('block'))
  assert.ok(r.visible.includes('block') && r.visible.includes('blocks') && r.visible.includes('marks'))
  const order = groupsOf().map((g) => g.id)
  assert.deepEqual(r.visible, order.filter((id) => r.visible.includes(id)))
})

test('the More menu lists folded groups from the most to the least useful', () => {
  const r = splitGroups(200, groupsOf())
  const prio = (id: ToolbarGroupId) => groupsOf().find((g) => g.id === id)!.priority
  const p = r.overflow.map(prio)
  assert.deepEqual(p, [...p].sort((a, b) => a - b))
  assert.ok(r.overflow.indexOf('color') < r.overflow.indexOf('align'))
})

test('the bar plus its More button always fits the width, down to the essentials', () => {
  const never = groupsOf().filter((g) => g.priority < 0).reduce((n, g) => n + groupWidth(g), 0) + MORE_WIDTH
  for (let w = 120; w < total; w += 17) {
    const r = splitGroups(w, groupsOf())
    const used = groupsOf().filter((g) => r.visible.includes(g.id)).reduce((n, g) => n + groupWidth(g), 0) + (r.overflow.length ? MORE_WIDTH : 0)
    assert.ok(used <= w || used <= never, `width ${w}`)
    assert.ok(r.visible.includes('block'))
  }
})

test('every viewport from a phone to a wide screen: touch buttons are 40px, desktop 32px', () => {
  assert.equal(buttonWidth('compact', true), 40)
  assert.equal(buttonWidth('comfortable', true), 40)
  assert.equal(buttonWidth('compact', false), 32)
  assert.ok(buttonWidth('comfortable', false) > 32)
  // 390px phone: something folds, something stays; 1440px: everything fits
  const phone = splitGroups(390 - 16 - 10, toolbarGroups(CFG, false, undefined, true), 40, 0, SEP_WIDTH_TOUCH)
  assert.ok(phone.overflow.length > 0 && phone.visible.includes('block') && phone.visible.includes('marks') && phone.visible.includes('blocks'))
  assert.ok(phone.overflow.includes('align') && phone.overflow.includes('insert'))
  assert.equal(splitGroups(1440 - 16, groupsOf(), 32, 38).overflow.length, 0)
})

test('hidden tools shrink or remove their group; the table group only exists in a table', () => {
  const cfg = cleanConfig({ hidden: ['bold', 'italic', 'underline', 'strike', 'code'] })
  assert.ok(!groupsOf(cfg).some((g) => g.id === 'marks'))
  assert.ok(!groupsOf().some((g) => g.id === 'table'))
  assert.ok(groupsOf(CFG, true).some((g) => g.id === 'table'))
  assert.deepEqual(groupTools('marks', cleanConfig({ hidden: ['bold'] })), ['italic', 'underline', 'strike', 'code'])
  const narrower = groupsOf(cleanConfig({ hidden: ['strike'] })).find((g) => g.id === 'marks')!.slots
  assert.equal(narrower, groupsOf().find((g) => g.id === 'marks')!.slots - 1)
  assert.ok(groupsOf(cleanConfig({ labels: false })).find((g) => g.id === 'block')!.slots < groupsOf().find((g) => g.id === 'block')!.slots)
})

test('groups follow the chosen order; a ribbon tab only lists its own groups', () => {
  const cfg = cleanConfig({ order: ['marks', 'history'] })
  assert.deepEqual(groupsOf(cfg).map((g) => g.id).slice(0, 2), ['marks', 'history'])
  assert.deepEqual(groupsOf(CFG, false, TAB_GROUPS.insert).map((g) => g.id), ['insert', 'blocks'])
  assert.deepEqual(ribbonTabs(false), ['home', 'insert'])
  assert.deepEqual(ribbonTabs(true), ['home', 'insert', 'table'])
  assert.equal(ribbonTabOr('table', false), 'home')
  assert.equal(ribbonTabOr('table', true), 'table')
})

test('ribbon tabs cover every group exactly once', () => {
  const all = Object.values(TAB_GROUPS).flat().sort()
  assert.deepEqual(all, [...ALL_GROUPS].sort())
  assert.deepEqual(ALL_TOOLS.length, new Set(ALL_TOOLS).size)
})

test('config: defaults, legacy and unknown stored values never throw', () => {
  assert.deepEqual(cleanConfig(undefined), DEFAULT_CONFIG)
  assert.deepEqual(cleanConfig(null), DEFAULT_CONFIG)
  assert.deepEqual(cleanConfig('nope'), DEFAULT_CONFIG)
  assert.deepEqual(cleanConfig([1, 2]), DEFAULT_CONFIG)
  assert.deepEqual(cleanConfig({}), DEFAULT_CONFIG)
  const c = cleanConfig({ style: 'neon', position: 'left', size: 'huge', labels: 'yes', order: ['marks', 'ghost', 'marks', 42], hidden: ['bold', 'ghost', 'bold'], animate: 'x', opacity: 9, reduce: 1, collapsed: null })
  assert.equal(c.style, 'bar')
  assert.equal(c.position, 'auto')
  assert.equal(c.size, 'compact')
  assert.equal(c.labels, true)
  assert.equal(c.animate, null)
  assert.equal(c.reduce, false)
  assert.equal(c.opacity, MAX_OPACITY)
  assert.deepEqual(c.hidden, ['bold'])
  assert.equal(c.order[0], 'marks')
  assert.deepEqual([...c.order].sort(), [...ALL_GROUPS].sort())
  assert.equal(cleanConfig({ opacity: -3 }).opacity, MIN_OPACITY)
  assert.equal(cleanConfig({ opacity: Number.NaN }).opacity, DEFAULT_CONFIG.opacity)
})

test('config: a group added after the preference was saved is appended', () => {
  const c = cleanConfig({ order: ['marks', 'history', 'block'] })
  assert.deepEqual(c.order.slice(0, 3), ['marks', 'history', 'block'])
  assert.equal(c.order.length, ALL_GROUPS.length)
})

test('config: serialization keeps only differences and round-trips', () => {
  assert.equal(serializeConfig(DEFAULT_CONFIG), null)
  const custom = cleanConfig({ style: 'ribbon', position: 'bottom', hidden: ['strike'], order: moveGroup(DEFAULT_ORDER, 'marks', -1), translucent: true, opacity: 0.4, animate: false })
  const out = serializeConfig(custom)!
  assert.deepEqual(Object.keys(out).sort(), ['animate', 'hidden', 'opacity', 'order', 'position', 'style', 'translucent'])
  assert.deepEqual(cleanConfig(out), custom)
  assert.deepEqual(cleanConfig(JSON.parse(JSON.stringify(out))), custom)
})

test('config: reordering and visibility helpers', () => {
  assert.deepEqual(moveGroup(['a', 'b', 'c'] as never, 'b' as never, -1), ['b', 'a', 'c'])
  assert.deepEqual(moveGroup(['a', 'b', 'c'] as never, 'a' as never, -1), ['a', 'b', 'c'])
  assert.deepEqual(moveGroup(['a', 'b', 'c'] as never, 'c' as never, 1), ['a', 'b', 'c'])
  const h = setGroupHidden([], 'lists', true)
  assert.equal(groupHidden(h, 'lists'), true)
  assert.deepEqual(setToolHidden(h, 'todo', false).includes('todo'), false)
  assert.equal(groupHidden(setToolHidden(h, 'todo', false), 'lists'), false)
  assert.deepEqual(setGroupHidden(h, 'lists', false), [])
})

test('style, position and animation resolve from the preference and the device', () => {
  assert.equal(effectiveStyle({ ...CFG, style: 'ribbon' }, false), 'ribbon')
  assert.equal(effectiveStyle({ ...CFG, style: 'ribbon' }, true), 'bar')
  assert.equal(effectivePosition(CFG, false), 'top')
  assert.equal(effectivePosition(CFG, true), 'bottom')
  assert.equal(effectivePosition({ ...CFG, position: 'bottom' }, false), 'bottom')
  assert.equal(effectivePosition({ ...CFG, position: 'top' }, true), 'top')
  assert.equal(animateOn(CFG, false), true)
  assert.equal(animateOn(CFG, true), false)
  assert.equal(animateOn({ ...CFG, animate: true }, true), true)
  assert.equal(animateOn({ ...CFG, animate: false }, false), false)
})

test('typing phase: reduced, translucent or hidden only while typing, opaque on hover or focus', () => {
  const all = { reduce: true, translucent: true, autoHide: true }
  assert.deepEqual(barPhase(false, false, all), { shrunk: false, faded: false, hidden: false })
  assert.deepEqual(barPhase(true, true, all), { shrunk: false, faded: false, hidden: false })
  assert.deepEqual(barPhase(true, false, all), { shrunk: true, faded: true, hidden: true })
  assert.deepEqual(barPhase(true, false, { reduce: true, translucent: false, autoHide: false }), { shrunk: true, faded: false, hidden: false })
  assert.deepEqual(barPhase(true, false, { reduce: false, translucent: true, autoHide: false }), { shrunk: false, faded: true, hidden: false })
  assert.deepEqual(barPhase(true, false, { reduce: false, translucent: false, autoHide: false }), { shrunk: false, faded: false, hidden: false })
})

test('recent colors: newest first, no duplicates, bounded', () => {
  assert.deepEqual(pushRecent([], 'red'), ['red'])
  assert.deepEqual(pushRecent(['red', 'blue'], 'blue'), ['blue', 'red'])
  assert.equal(pushRecent(['a', 'b', 'c', 'd', 'e'], 'f').length, 5)
  assert.equal(pushRecent(['a', 'b', 'c', 'd', 'e'], 'f')[0], 'f')
})

test('palette: 11 theme keys for text and highlight, no raw values, defined in both themes', () => {
  assert.equal(TEXT_COLORS.length, 11)
  assert.equal(new Set(TEXT_COLORS).size, 11)
  for (const c of TEXT_COLORS) { assert.equal(cleanColor(c), c); assert.equal(cleanHighlight(c), c) }
  assert.equal(cleanHighlight('#ff0'), null)
  assert.equal(cleanHighlight('yellow; background:url(x)'), null)
  const css = readFileSync(new URL('../app/assets/css/tokens.css', import.meta.url), 'utf8')
  const prose = readFileSync(new URL('../app/assets/css/prose.css', import.meta.url), 'utf8')
  for (const c of TEXT_COLORS) {
    assert.equal(css.split(`--pal-${c}:`).length, 3, `${c} text in dark and light`)
    assert.equal(css.split(`--pal-${c}-bg:`).length, 3, `${c} highlight in dark and light`)
    assert.ok(prose.includes(`[data-text-color='${c}']`) && prose.includes(`mark[data-highlight='${c}']`))
  }
})

function lum(hex: string) {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4))
  return 0.2126 * c[0]! + 0.7152 * c[1]! + 0.0722 * c[2]!
}
test('palette: every text color has WCAG AA contrast (4.5:1) on the page background in light and dark', () => {
  const css = readFileSync(new URL('../app/assets/css/tokens.css', import.meta.url), 'utf8')
  const dark = css.slice(0, css.indexOf('html.light'))
  const light = css.slice(css.indexOf('html.light'))
  const bg = (part: string) => /--bg: (#[0-9a-f]{6})/.exec(part)![1]!
  for (const [part, name] of [[dark, 'dark'], [light, 'light']] as const) {
    for (const c of TEXT_COLORS) {
      const fg = new RegExp(`--pal-${c}: (#[0-9a-f]{6})`).exec(part)![1]!
      const [a, b] = [lum(fg), lum(bg(part))].sort((x, y) => y - x)
      assert.ok((a! + 0.05) / (b! + 0.05) >= 4.5, `${c} in ${name}`)
    }
  }
})

test('insert menu reuses the slash list without repeating what the bar already has', () => {
  const slash = ['p', 'h1', 'ul', 'toggle', 'info', 'table', 'image', 'math', 'mathi', 'mermaid', 'embed', 'date'].map((id) => ({ id }))
  assert.deepEqual(insertItems(slash).map((i) => i.id), ['toggle', 'info', 'mathi', 'mermaid', 'embed', 'date'])
  assert.ok(TOOLBAR_COVERED.includes('h1') && !TOOLBAR_COVERED.includes('mermaid' as never))
})

function source(active: Record<string, Record<string, unknown> | true>, attrs: Record<string, Record<string, unknown>> = {}, can: Partial<ToolbarSource['can']> = {}): ToolbarSource {
  return {
    isActive: (n, a) => {
      const v = active[n]
      if (!v) return false
      return v === true || !a || Object.entries(a).every(([k, x]) => v[k] === x)
    },
    getAttributes: (n) => attrs[n] ?? {},
    can: { undo: () => false, redo: () => false, indent: () => false, outdent: () => false, mergeCells: () => true, splitCell: () => true, ...can },
  }
}

test('state mirrors the selection', () => {
  const s = toolbarState(source({ bold: true, heading: { level: 2 }, bulletList: true }, { heading: { textAlign: 'center' }, textColor: { color: 'red' } }, { undo: () => true }))
  assert.equal(s.bold, true)
  assert.equal(s.italic, false)
  assert.equal(s.block, 'h2')
  assert.equal(s.bulletList, true)
  assert.equal(s.align, 'center')
  assert.equal(s.color, 'red')
  assert.equal(s.canUndo, true)
  assert.equal(s.canRedo, false)
})

test('code block and quote win over paragraph; defaults without selection data', () => {
  assert.equal(toolbarState(source({ codeBlock: true })).block, 'code')
  assert.equal(toolbarState(source({ blockquote: true })).block, 'quote')
  assert.deepEqual(toolbarState(source({})), EMPTY_STATE)
})

test('table actions are only asked inside a table', () => {
  let asked = 0
  const can = { mergeCells: () => (asked++, true), splitCell: () => (asked++, true) }
  assert.equal(toolbarState(source({}, {}, can)).canMerge, false)
  assert.equal(asked, 0)
  const s = toolbarState(source({ table: true }, {}, can))
  assert.equal(s.inTable, true)
  assert.equal(s.canMerge && s.canSplit, true)
})

test('sameState lets a keystroke that changes nothing skip the redraw', () => {
  const a = toolbarState(source({ bold: true }))
  assert.equal(sameState(a, toolbarState(source({ bold: true }))), true)
  assert.equal(sameState(a, toolbarState(source({ bold: true, italic: true }))), false)
})

test('preference: shown unless explicitly hidden', () => {
  assert.equal(toolbarVisible(undefined), true)
  assert.equal(toolbarVisible(null), true)
  assert.equal(toolbarVisible(true), true)
  assert.equal(toolbarVisible(false), false)
})

test('roving focus wraps and honours Home and End', () => {
  assert.equal(rovingNext(0, 5, 'ArrowRight'), 1)
  assert.equal(rovingNext(4, 5, 'ArrowRight'), 0)
  assert.equal(rovingNext(0, 5, 'ArrowLeft'), 4)
  assert.equal(rovingNext(2, 5, 'Home'), 0)
  assert.equal(rovingNext(2, 5, 'End'), 4)
  assert.equal(rovingNext(2, 5, 'a'), 2)
  assert.equal(rovingNext(0, 0, 'ArrowRight'), -1)
})

test('keyboard inset follows the visual viewport', () => {
  assert.equal(keyboardInset(800, { height: 500, offsetTop: 0 }), 300)
  assert.equal(keyboardInset(800, { height: 800, offsetTop: 0 }), 0)
  assert.equal(keyboardInset(800, { height: 500, offsetTop: 40 }), 260)
  assert.equal(keyboardInset(800, null), 0)
})

test('colors and alignment only accept known values', () => {
  assert.equal(cleanColor('red'), 'red')
  assert.equal(cleanColor('#ff0000'), null)
  assert.equal(cleanColor('url(x)'), null)
  assert.equal(cleanAlign('center'), 'center')
  assert.equal(cleanAlign('left'), null)
  assert.equal(cleanAlign('foo'), null)
})
