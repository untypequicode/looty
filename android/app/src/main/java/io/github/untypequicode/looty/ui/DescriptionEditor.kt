package io.github.untypequicode.looty.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.FormatIndentDecrease
import androidx.compose.material.icons.automirrored.filled.FormatIndentIncrease
import androidx.compose.material.icons.automirrored.filled.FormatListBulleted
import androidx.compose.material.icons.filled.*
import androidx.compose.material.icons.outlined.Code
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.input.key.*
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.filterNotNull

/**
 * État de l'éditeur de description : la liste de blocs, le bloc en cours d'édition et
 * l'enregistrement (une seconde après la dernière frappe, et en quittant l'écran).
 */
class DescEditorState(html: String?, private val onSave: (String?) -> Unit) {
    val blocks = mutableStateListOf<EBlock>().apply { addAll(RichDoc.parse(html)) }
    var focusedId by mutableStateOf<Long?>(null)
    /** Bloc à activer après la prochaine composition (identifiant, position du curseur). */
    var pendingFocus by mutableStateOf<Pair<Long, Int>?>(null)
    var version by mutableIntStateOf(0); private set
    private var saved: String? = RichDoc.toHtml(blocks)
    val focusers = mutableMapOf<Long, FocusRequester>()

    val focused: EBlock? get() = focusedId?.let { id -> blocks.firstOrNull { it.id == id } }
    val isEmpty get() = blocks.size == 1 && blocks[0].type == BType.P && blocks[0].text.text.isEmpty()

    /** Nouvelle version reçue du serveur : remplace le contenu si rien n'est en cours d'édition. */
    fun external(html: String?) {
        if (focusedId != null) return
        val parsed = RichDoc.parse(html)
        val h = RichDoc.toHtml(parsed)
        if (h == saved || h == RichDoc.toHtml(blocks)) return
        blocks.clear(); blocks.addAll(parsed); saved = h
    }

    fun save() {
        val h = RichDoc.toHtml(blocks)
        if (h != saved) { saved = h; onSave(h) }
    }

    private fun changed() { version++ }
    private fun index(id: Long) = blocks.indexOfFirst { it.id == id }
    private fun set(i: Int, b: EBlock) { blocks[i] = b; changed() }

    fun onText(id: Long, v: TextFieldValue) {
        val i = index(id).takeIf { it >= 0 } ?: return
        val old = blocks[i]
        if (old.type == BType.CODE || v.text.count { it == '\n' } <= old.text.text.count { it == '\n' }) {
            if (v.text == old.text.text) { blocks[i] = old.copy(text = v); return }
            set(i, shortcut(old, v)); return
        }
        // Retour à la ligne : un nouveau bloc (même type de liste), ou fin de la liste sur un élément vide.
        val lines = v.text.split('\n')
        if (old.type != BType.P && lines.size == 2 && lines.all { it.isEmpty() }) {
            set(i, if (old.isList && old.indent > 0) old.copy(indent = old.indent - 1, text = TextFieldValue("")) else old.copy(type = BType.P, indent = 0, text = TextFieldValue("")))
            return
        }
        val nextType = when (old.type) { BType.BULLET, BType.NUMBER, BType.CHECK, BType.QUOTE -> old.type; else -> BType.P }
        var cursorLeft = v.selection.start
        var focus: Pair<Long, Int>? = null
        lines.forEachIndexed { n, line ->
            val nb = if (n == 0) old.copy(text = TextFieldValue(line, TextRange(line.length)))
            else EBlock(RichDoc.newId(), nextType, TextFieldValue(line, TextRange(0)), old.indent)
            if (n == 0) blocks[i] = nb else blocks.add(i + n, nb)
            if (focus == null && cursorLeft <= line.length) focus = nb.id to cursorLeft
            cursorLeft -= line.length + 1
        }
        pendingFocus = focus ?: (blocks[i + lines.size - 1].id to 0)
        changed()
    }

    /** Raccourcis en début de paragraphe : « - », « 1. », « [] », « # », « > », « ``` ». */
    private fun shortcut(old: EBlock, v: TextFieldValue): EBlock {
        if (old.type != BType.P) return old.copy(text = v)
        val rules = listOf("- " to BType.BULLET, "* " to BType.BULLET, "1. " to BType.NUMBER, "[] " to BType.CHECK, "[ ] " to BType.CHECK,
            "### " to BType.H3, "## " to BType.H2, "# " to BType.H1, "> " to BType.QUOTE, "```" to BType.CODE)
        for ((p, t) in rules) if (v.text.startsWith(p) && !old.text.text.startsWith(p) && v.selection.start == p.length) {
            val rest = v.text.removePrefix(p)
            return old.copy(type = t, text = TextFieldValue(rest, TextRange(0)))
        }
        return old.copy(text = v)
    }

    /** Effacement au début d'un bloc : enlève le type ou le retrait, sinon fusionne avec le bloc précédent. */
    fun backspaceAtStart(id: Long): Boolean {
        val i = index(id).takeIf { it >= 0 } ?: return false
        val b = blocks[i]
        if (b.indent > 0) { set(i, b.copy(indent = b.indent - 1)); return true }
        if (b.type != BType.P) { set(i, b.copy(type = BType.P, checked = false)); return true }
        if (i == 0) return false
        val prev = blocks[i - 1]
        if (prev.type == BType.RAW) {
            if (b.text.text.isNotEmpty()) return true
            blocks.removeAt(i); changed()
            blocks.subList(0, i).lastOrNull { it.type != BType.RAW }?.let { pendingFocus = it.id to it.text.text.length }
            return true
        }
        val join = prev.text.text.length
        blocks[i - 1] = prev.copy(text = TextFieldValue(prev.text.text + b.text.text, TextRange(join)))
        blocks.removeAt(i)
        pendingFocus = prev.id to join
        changed()
        return true
    }

    fun toggleCheck(id: Long) { val i = index(id); if (i >= 0) set(i, blocks[i].copy(checked = !blocks[i].checked)) }
    fun remove(id: Long) { val i = index(id); if (i >= 0) { blocks.removeAt(i); if (blocks.isEmpty() || blocks.last().type != BType.P) blocks.add(RichDoc.block(BType.P)); changed() } }

    // --- Barre d'outils (bloc actif) ---

    fun setType(t: BType) {
        val i = focusedId?.let(::index)?.takeIf { it >= 0 } ?: return
        val b = blocks[i]
        set(i, if (b.type == t) b.copy(type = BType.P, indent = 0) else b.copy(type = t, checked = false, indent = if (t == BType.BULLET || t == BType.NUMBER || t == BType.CHECK) b.indent else 0))
    }

    fun indent(delta: Int) {
        val i = focusedId?.let(::index)?.takeIf { it >= 0 } ?: return
        val b = blocks[i]
        if (!b.isList) return
        val max = if (i > 0 && blocks[i - 1].isList) blocks[i - 1].indent + 1 else 0
        set(i, b.copy(indent = (b.indent + delta).coerceIn(0, minOf(max, 4))))
    }

    /** Gras, italique… : entoure la sélection (ou retire la mise en forme si elle y est déjà). */
    fun wrap(marker: String) {
        val i = focusedId?.let(::index)?.takeIf { it >= 0 } ?: return
        val b = blocks[i]
        if (b.type == BType.CODE) return
        val t = b.text.text
        val sel = b.text.selection
        val (s, e) = sel.min to sel.max
        val v = if (t.substring(0, s).endsWith(marker) && t.substring(e).startsWith(marker)) {
            TextFieldValue(t.substring(0, s - marker.length) + t.substring(s, e) + t.substring(e + marker.length), TextRange(s - marker.length, e - marker.length))
        } else {
            TextFieldValue(t.substring(0, s) + marker + t.substring(s, e) + marker + t.substring(e), if (s == e) TextRange(s + marker.length) else TextRange(s + marker.length, e + marker.length))
        }
        set(i, b.copy(text = v))
    }

    /** Bloc où insérer un lien (la boîte de dialogue prend le focus). */
    var linkFor by mutableStateOf<Long?>(null)

    fun link(id: Long, url: String) {
        val i = index(id).takeIf { it >= 0 } ?: return
        val b = blocks[i]
        val t = b.text.text
        val sel = b.text.selection
        val label = t.substring(sel.min, sel.max).ifEmpty { RichDoc.escape(url) }
        val ins = "[$label](${url.trim().replace(" ", "%20")})"
        set(i, b.copy(text = TextFieldValue(t.substring(0, sel.min) + ins + t.substring(sel.max), TextRange(sel.min + ins.length))))
        pendingFocus = id to sel.min + ins.length
    }
}

@Composable
fun rememberDescEditor(key: String, html: String?, onSave: (String?) -> Unit): DescEditorState {
    val save by rememberUpdatedState(onSave)
    val st = remember(key) { DescEditorState(html) { save(it) } }
    LaunchedEffect(st, html) { st.external(html) }
    LaunchedEffect(st, st.version) { if (st.version > 0) { delay(1000); st.save() } }
    DisposableEffect(st) { onDispose { st.save() } }
    return st
}

/** Description d'une tâche, modifiable sur place, bloc par bloc. */
@OptIn(androidx.compose.foundation.ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
fun DescriptionEditor(state: AppState, st: DescEditorState, modifier: Modifier = Modifier) {
    val scheme = MaterialTheme.colorScheme
    val markup = remember(scheme) { RichDoc.Markup(scheme.outline, scheme.primary, scheme.surfaceContainerHighest) }
    val body = MaterialTheme.typography.bodyLarge.copy(color = scheme.onSurface)
    val placeholder = stringResource(R.string.description_placeholder)
    var rawMenu by remember { mutableStateOf<Long?>(null) }

    // Clavier fermé (retour) : fin de l'édition, la barre de mise en forme disparaît.
    val imeVisible = WindowInsets.isImeVisible
    val focusManager = LocalFocusManager.current
    var imeWasVisible by remember { mutableStateOf(false) }
    LaunchedEffect(imeVisible) {
        if (imeWasVisible && !imeVisible && st.focusedId != null) focusManager.clearFocus()
        imeWasVisible = imeVisible
    }

    // Curseur à placer dans un autre bloc (après Entrée, effacement…) : une fois ce bloc composé.
    LaunchedEffect(st) {
        snapshotFlow { st.pendingFocus }.filterNotNull().collect { (id, pos) ->
            val i = st.blocks.indexOfFirst { it.id == id }
            if (i >= 0) {
                val b = st.blocks[i]
                st.blocks[i] = b.copy(text = b.text.copy(selection = TextRange(pos.coerceIn(0, b.text.text.length))))
                withFrameNanos { }
                runCatching { st.focusers[id]?.requestFocus() }
            }
            st.pendingFocus = null
        }
    }

    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        var number = IntArray(6)
        st.blocks.forEach { b ->
            key(b.id) {
                // Numérotation : recommence après tout bloc qui n'est pas un élément numéroté plus profond.
                if (b.type == BType.NUMBER) { number[b.indent.coerceAtMost(5)]++; for (l in b.indent + 1 until 6) number[l] = 0 } else if (!b.isList || b.indent == 0) number = IntArray(6)
                val n = number[b.indent.coerceAtMost(5)]
                if (b.type == BType.RAW) {
                    Box(Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).combinedClickable(onClick = {}, onLongClick = { rawMenu = b.id }).padding(vertical = 2.dp)) {
                        HtmlBlocks(state, b.html)
                        DropdownMenu(expanded = rawMenu == b.id, onDismissRequest = { rawMenu = null }) {
                            DropdownMenuItem(text = { Text(stringResource(R.string.delete)) }, leadingIcon = { Icon(Icons.Outlined.Delete, null) }, onClick = { rawMenu = null; st.remove(b.id) })
                        }
                    }
                } else {
                val style = when (b.type) {
                    BType.H1 -> MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.SemiBold)
                    BType.H2 -> MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.SemiBold)
                    BType.H3 -> MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold)
                    BType.CODE -> TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp)
                    BType.QUOTE -> body.copy(color = scheme.onSurfaceVariant)
                    BType.CHECK -> if (b.checked) body.copy(color = scheme.onSurfaceVariant, textDecoration = TextDecoration.LineThrough) else body
                    else -> body
                }.let { if (it.color == androidx.compose.ui.graphics.Color.Unspecified) it.copy(color = scheme.onSurface) else it }
                val fr = remember(b.id) { st.focusers.getOrPut(b.id) { FocusRequester() } }
                Row(Modifier.fillMaxWidth().padding(start = (b.indent * 22).dp).height(IntrinsicSize.Min), verticalAlignment = Alignment.Top) {
                    when (b.type) {
                        BType.BULLET -> Text(listOf("•", "◦", "▪")[b.indent % 3], style = body, modifier = Modifier.width(22.dp))
                        BType.NUMBER -> Text("$n.", style = body, modifier = Modifier.widthIn(min = 26.dp).padding(end = 4.dp))
                        BType.CHECK -> Checkbox(checked = b.checked, onCheckedChange = { st.toggleCheck(b.id) }, modifier = Modifier.size(24.dp).padding(end = 6.dp))
                        BType.QUOTE -> Box(Modifier.padding(end = 10.dp).width(3.dp).fillMaxHeight().background(scheme.outlineVariant))
                        else -> {}
                    }
                    val field = Modifier.weight(1f).focusRequester(fr)
                        .onFocusChanged { f -> if (f.isFocused) st.focusedId = b.id else if (st.focusedId == b.id) st.focusedId = null }
                        .onPreviewKeyEvent { e ->
                            val sel = b.text.selection
                            if (e.type == KeyEventType.KeyDown && e.key == Key.Backspace && sel.collapsed && sel.start == 0) st.backspaceAtStart(b.id) else false
                        }
                    val content: @Composable () -> Unit = {
                        BasicTextField(
                            value = b.text, onValueChange = { st.onText(b.id, it) },
                            textStyle = style, cursorBrush = SolidColor(scheme.primary),
                            visualTransformation = if (b.type == BType.CODE) androidx.compose.ui.text.input.VisualTransformation.None else markup,
                            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                            modifier = field.padding(vertical = 2.dp),
                            decorationBox = { inner ->
                                Box {
                                    if (st.isEmpty) Text(placeholder, style = style.copy(color = scheme.onSurfaceVariant))
                                    inner()
                                }
                            },
                        )
                    }
                    if (b.type == BType.CODE) Surface(color = scheme.surfaceContainerHighest, shape = RoundedCornerShape(8.dp), modifier = Modifier.weight(1f)) {
                        Box(Modifier.horizontalScroll(rememberScrollState()).padding(10.dp)) { content() }
                    } else content()
                }
                }
            }
        }
    }

    st.linkFor?.let { id ->
        var url by remember { mutableStateOf("https://") }
        val back = { st.linkFor = null; st.pendingFocus = id to (st.blocks.firstOrNull { it.id == id }?.text?.selection?.max ?: 0) }
        AlertDialog(
            onDismissRequest = back,
            title = { Text(stringResource(R.string.fmt_link)) },
            text = { OutlinedTextField(url, { url = it }, singleLine = true, label = { Text(stringResource(R.string.link_address)) }) },
            confirmButton = { TextButton(enabled = url.length > 8, onClick = { st.linkFor = null; st.link(id, url) }) { Text(stringResource(R.string.add)) } },
            dismissButton = { TextButton(onClick = back) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** Barre de mise en forme, au-dessus du clavier pendant l'édition de la description. */
@Composable
fun DescEditorToolbar(st: DescEditorState) {
    val b = st.focused ?: return
    Surface(tonalElevation = 3.dp, shadowElevation = 3.dp) {
        Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).navigationBarsPadding().imePadding().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            // Titre : texte → titre 1 → titre 2 → titre 3 → texte.
            val heading = b.type == BType.H1 || b.type == BType.H2 || b.type == BType.H3
            val headingLabel = stringResource(R.string.fmt_heading)
            IconToggleButton(checked = heading, onCheckedChange = {
                st.setType(when (b.type) { BType.H1 -> BType.H2; BType.H2 -> BType.H3; BType.H3 -> BType.H3; else -> BType.H1 })
            }, modifier = Modifier.semantics { contentDescription = headingLabel }) {
                Text(when (b.type) { BType.H1 -> "H1"; BType.H2 -> "H2"; BType.H3 -> "H3"; else -> "H" }, fontWeight = FontWeight.Bold)
            }
            ToolButton(Icons.AutoMirrored.Filled.FormatListBulleted, R.string.fmt_bullets, b.type == BType.BULLET) { st.setType(BType.BULLET) }
            ToolButton(Icons.Filled.FormatListNumbered, R.string.fmt_numbers, b.type == BType.NUMBER) { st.setType(BType.NUMBER) }
            ToolButton(Icons.Filled.Checklist, R.string.fmt_checklist, b.type == BType.CHECK) { st.setType(BType.CHECK) }
            if (b.isList) {
                ToolButton(Icons.AutoMirrored.Filled.FormatIndentDecrease, R.string.fmt_outdent, false) { st.indent(-1) }
                ToolButton(Icons.AutoMirrored.Filled.FormatIndentIncrease, R.string.fmt_indent, false) { st.indent(1) }
            }
            ToolButton(Icons.Filled.FormatQuote, R.string.fmt_quote, b.type == BType.QUOTE) { st.setType(BType.QUOTE) }
            ToolButton(Icons.Filled.DataObject, R.string.fmt_code_block, b.type == BType.CODE) { st.setType(BType.CODE) }
            if (b.type != BType.CODE) {
                VerticalDivider(Modifier.height(24.dp).padding(horizontal = 4.dp))
                ToolButton(Icons.Filled.FormatBold, R.string.fmt_bold, false) { st.wrap("**") }
                ToolButton(Icons.Filled.FormatItalic, R.string.fmt_italic, false) { st.wrap("*") }
                ToolButton(Icons.Filled.FormatUnderlined, R.string.fmt_underline, false) { st.wrap("__") }
                ToolButton(Icons.Filled.FormatStrikethrough, R.string.fmt_strike, false) { st.wrap("~~") }
                ToolButton(Icons.Outlined.Code, R.string.fmt_code, false) { st.wrap("`") }
                ToolButton(Icons.Filled.Link, R.string.fmt_link, false) { st.linkFor = b.id }
            }
        }
    }
}

@Composable
private fun ToolButton(icon: androidx.compose.ui.graphics.vector.ImageVector, label: Int, on: Boolean, onClick: () -> Unit) {
    IconToggleButton(checked = on, onCheckedChange = { onClick() }) { Icon(icon, stringResource(label)) }
}
