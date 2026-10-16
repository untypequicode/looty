package io.github.untypequicode.looty.ui

import android.content.Intent
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.*
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.*
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.BaselineShift
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Page
import org.jsoup.Jsoup
import org.jsoup.nodes.Element
import org.jsoup.nodes.Node
import org.jsoup.nodes.TextNode

// ---------------------------------------------------------------------------
// Document (HTML de l'éditeur) → blocs
// ---------------------------------------------------------------------------

sealed interface Block
data class Heading(val level: Int, val text: AnnotatedString) : Block
data class Para(val text: AnnotatedString) : Block
data class Entry(val checked: Boolean?, val blocks: List<Block>)
data class ListBlock(val ordered: Boolean, val items: List<Entry>) : Block
data class Quote(val kind: String?, val blocks: List<Block>) : Block
data class Code(val lang: String?, val text: String) : Block
data object Rule : Block
data class Img(val src: String, val alt: String) : Block
data class Cell(val header: Boolean, val text: AnnotatedString)
data class Table(val rows: List<List<Cell>>) : Block
data class MathBlock(val latex: String) : Block
/** Contenu que l'éditeur sait afficher mais pas l'application (schéma, dessin, intégration, PDF). */
data class Special(val kind: String, val source: String?) : Block
data class Toggle(val summary: AnnotatedString, val blocks: List<Block>) : Block

/** Couleurs du thème, nécessaires pour styler les liens et le code dans le texte. */
class ReaderColors(val link: Color, val codeBackground: Color, val mark: Color)

/**
 * Lecture native d'un document : l'HTML enregistré par l'éditeur du site devient une liste
 * de blocs affichés en Compose. Les formules sont montrées en LaTeX et les schémas / dessins
 * renvoient vers l'éditeur.
 */
object ReaderParser {
    private val inlineTags = setOf("a", "strong", "b", "em", "i", "u", "s", "del", "code", "mark", "span", "br", "sub", "sup", "small")

    fun parse(html: String, colors: ReaderColors, onLink: (String) -> Unit): List<Block> =
        Ctx(colors, onLink).blocks(Jsoup.parseBodyFragment(html).body().childNodes())

    private class Ctx(val colors: ReaderColors, val onLink: (String) -> Unit) {
        fun isInline(n: Node) = n is TextNode || (n is Element && n.tagName() in inlineTags)

        fun blocks(nodes: List<Node>): List<Block> {
            val out = mutableListOf<Block>()
            val pending = mutableListOf<Node>()
            fun flush() {
                if (pending.isEmpty()) return
                val text = inlineOf(pending)
                if (text.text.isNotBlank()) out += Para(text)
                pending.clear()
            }
            for (n in nodes) {
                if (isInline(n)) { pending += n; continue }
                flush()
                if (n is Element) element(n, out)
            }
            flush()
            return out
        }

        fun inlineOf(nodes: List<Node>): AnnotatedString = buildAnnotatedString { nodes.forEach { inline(it) } }

        /** Texte d'un élément contenant éventuellement des paragraphes (cellule, résumé). */
        fun flat(el: Element): AnnotatedString = buildAnnotatedString {
            var first = true
            for (n in el.childNodes()) {
                if (n is Element && !isInline(n)) {
                    if (!first) append("\n")
                    append(flat(n)); first = false
                } else { inline(n); first = false }
            }
        }

        fun AnnotatedString.Builder.inline(n: Node) {
            if (n is TextNode) { append(n.wholeText); return }
            val el = n as? Element ?: return
            fun styled(s: SpanStyle) = withStyle(s) { el.childNodes().forEach { inline(it) } }
            when (el.tagName()) {
                "br" -> append("\n")
                "strong", "b" -> styled(SpanStyle(fontWeight = FontWeight.Bold))
                "em", "i" -> styled(SpanStyle(fontStyle = FontStyle.Italic))
                "u" -> styled(SpanStyle(textDecoration = TextDecoration.Underline))
                "s", "del" -> styled(SpanStyle(textDecoration = TextDecoration.LineThrough))
                "code" -> styled(SpanStyle(fontFamily = FontFamily.Monospace, background = colors.codeBackground))
                "mark" -> styled(SpanStyle(background = colors.mark))
                "sub" -> styled(SpanStyle(baselineShift = BaselineShift.Subscript, fontSize = 12.sp))
                "sup" -> styled(SpanStyle(baselineShift = BaselineShift.Superscript, fontSize = 12.sp))
                "a" -> {
                    val href = el.attr("href")
                    if (href.isBlank()) el.childNodes().forEach { inline(it) }
                    else withLink(LinkAnnotation.Clickable(href, TextLinkStyles(SpanStyle(color = colors.link, textDecoration = TextDecoration.Underline))) { onLink(href) }) {
                        el.childNodes().forEach { inline(it) }
                    }
                }
                "span" -> if (el.attr("data-type") == "math-inline") {
                    withStyle(SpanStyle(fontFamily = FontFamily.Monospace, fontStyle = FontStyle.Italic, background = colors.codeBackground)) { append(el.attr("latex")) }
                } else el.childNodes().forEach { inline(it) }
                else -> el.childNodes().forEach { inline(it) }
            }
        }

        fun element(el: Element, out: MutableList<Block>) {
            when (val tag = el.tagName()) {
                "h1", "h2", "h3", "h4", "h5", "h6" -> out += Heading(tag[1] - '0', flat(el))
                "ul", "ol" -> out += ListBlock(tag == "ol", el.children().filter { it.tagName() == "li" }.map { li ->
                    val task = li.attr("data-type") == "taskItem"
                    Entry(if (task) li.attr("data-checked") == "true" else null, blocks(li.childNodes().filter { !(it is Element && it.tagName() == "label") }))
                })
                "blockquote" -> out += Quote(el.attr("data-notice-type").ifBlank { null }, blocks(el.childNodes()))
                "pre" -> {
                    val code = el.selectFirst("code")
                    out += Code(code?.classNames()?.firstOrNull { it.startsWith("language-") }?.removePrefix("language-"), (code ?: el).wholeText().trimEnd('\n'))
                }
                "hr" -> out += Rule
                "img" -> out += Img(el.attr("src"), el.attr("alt"))
                "table" -> out += Table(el.select("tr").map { tr -> tr.children().filter { it.tagName() == "td" || it.tagName() == "th" }.map { Cell(it.tagName() == "th", flat(it)) } })
                "details" -> {
                    val summary = el.selectFirst("summary")
                    out += Toggle(summary?.let { flat(it) } ?: AnnotatedString(""), blocks(el.childNodes().filter { it !== summary }))
                }
                "div" -> when (val type = el.attr("data-type")) {
                    "math-block" -> out += MathBlock(el.attr("latex"))
                    "mermaid" -> out += Special("mermaid", el.attr("code"))
                    "excalidraw", "embed", "pdf" -> out += Special(type, el.attr("src").ifBlank { null })
                    else -> out += blocks(el.childNodes())
                }
                "label", "script", "style", "colgroup" -> {}
                else -> out += blocks(el.childNodes())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Écran
// ---------------------------------------------------------------------------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReaderScreen(state: AppState, pageId: String, onBack: () -> Unit, onRoute: (String) -> Unit) {
    val ctx = LocalContext.current
    val ws = state.ws ?: return
    var page by remember(pageId) { mutableStateOf<Page?>(null) }
    var failed by remember(pageId) { mutableStateOf(false) }
    var actions by remember { mutableStateOf(false) }
    LaunchedEffect(pageId, ws) {
        failed = false
        page = state.pageDoc(pageId)
        if (page == null) failed = true else state.recordView(pageId)
    }
    // La liste fait foi pour ce qui change ici (titre, icône, favori, emplacement).
    val listed = state.pages.data?.firstOrNull { it.id == pageId }
    val p = page?.let { d -> listed?.let { l -> d.copy(title = l.title, icon = l.icon, is_favorite = l.is_favorite, parent_id = l.parent_id, share_token = l.share_token, is_public = l.is_public) } ?: d }
    val canEdit = p?.can_edit == true || listed?.can_edit == true
    val comments = state.docCommentCounts[pageId] ?: 0
    val colors = ReaderColors(MaterialTheme.colorScheme.primary, MaterialTheme.colorScheme.surfaceContainerHighest, Color(0x66FFD54F))
    val blocks = remember(page?.content, colors.link) {
        page?.content?.takeIf { it.isNotBlank() }?.let { html -> ReaderParser.parse(html, colors) { href -> openLink(ctx, state, href) } }.orEmpty()
    }
    // Chemin (pages parentes) et sous-pages, d'après la liste des documents.
    val all = state.pages.data.orEmpty()
    val path = remember(all, p?.parent_id) {
        buildList { var cur = p?.parent_id; var guard = 0; while (cur != null && guard++ < 20) { val x = all.firstOrNull { it.id == cur } ?: break; add(0, x); cur = x.parent_id } }
    }
    val children = remember(all, pageId) { all.filter { it.parent_id == pageId }.sortedBy { it.position } }
    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    if (p != null) {
                        IconButton(onClick = { state.toggleFavorite(listed ?: p) }) {
                            Icon(if (p.is_favorite == true) Icons.Filled.Star else Icons.Outlined.StarOutline, stringResource(if (p.is_favorite == true) R.string.remove_favorite else R.string.add_favorite),
                                tint = if (p.is_favorite == true) Color(0xFFEDA100) else LocalContentColor.current)
                        }
                        IconButton(onClick = { onRoute("doc/$pageId/comments") }) {
                            BadgedBox(badge = { if (comments > 0) Badge { Text("$comments") } }) { Icon(Icons.Outlined.ChatBubbleOutline, stringResource(R.string.comments)) }
                        }
                    }
                    IconButton(onClick = { openWeb(ctx, Paths.doc(ws, pageId)) }) {
                        Icon(if (canEdit) Icons.Outlined.Edit else Icons.Outlined.OpenInNew, stringResource(if (canEdit) R.string.edit else R.string.open_in_browser_view))
                    }
                    if (p != null) IconButton(onClick = { actions = true }) { Icon(Icons.Outlined.MoreVert, null) }
                },
            )
        },
    ) { padding ->
        when {
            failed -> Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(stringResource(R.string.reader_failed), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    TextButton(onClick = { openWeb(ctx, Paths.doc(ws, pageId)) }) { Text(stringResource(R.string.edit_in_editor)) }
                }
            }
            p == null -> Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            else -> SelectionContainer {
                LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    if (path.isNotEmpty()) item {
                        Row(Modifier.horizontalScroll(rememberScrollState()), verticalAlignment = Alignment.CenterVertically) {
                            path.forEachIndexed { i, a ->
                                if (i > 0) Text(" › ", color = MaterialTheme.colorScheme.onSurfaceVariant)
                                Text(listOfNotNull(a.icon, a.title.ifBlank { stringResource(R.string.untitled) }).joinToString(" "), style = MaterialTheme.typography.labelLarge,
                                    color = MaterialTheme.colorScheme.primary, maxLines = 1, modifier = Modifier.clickable { onRoute("doc/${a.id}") })
                            }
                        }
                    }
                    item {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            if (!p.icon.isNullOrBlank()) { Text(p.icon, style = MaterialTheme.typography.headlineMedium); Spacer(Modifier.width(10.dp)) }
                            Text(p.title.ifBlank { stringResource(R.string.untitled) }, style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
                        }
                    }
                    if (blocks.isEmpty()) item {
                        Text(stringResource(R.string.reader_empty), color = MaterialTheme.colorScheme.onSurfaceVariant)
                        if (canEdit) TextButton(onClick = { openWeb(ctx, Paths.doc(ws, pageId)) }, contentPadding = PaddingValues(0.dp)) { Text(stringResource(R.string.edit_in_editor)) }
                    }
                    items(blocks) { BlockView(state, it, pageId) }
                    if (children.isNotEmpty()) {
                        item { HorizontalDivider(Modifier.padding(top = 16.dp)); Text(stringResource(R.string.subpages_title), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 12.dp)) }
                        items(children, key = { it.id }) { c ->
                            Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).clickable { onRoute("doc/${c.id}") }.padding(vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                                DocIcon(c); Spacer(Modifier.width(10.dp))
                                Text(c.title.ifBlank { stringResource(R.string.untitled) }, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Icon(Icons.Outlined.ChevronRight, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                    item { Spacer(Modifier.height(48.dp)) }
                }
            }
        }
    }
    if (actions && p != null) DocActionsSheet(state, listed ?: p, onDismiss = { actions = false }, onOpen = null,
        onComments = { onRoute("doc/$pageId/comments") }, onHistory = { onRoute("doc/$pageId/history") }, onDeleted = { onBack() })
}

/** Lien du document : page de Looty (écran natif si possible), sinon le navigateur. */
private fun openLink(ctx: android.content.Context, state: AppState, href: String) {
    val server = runCatching { state.api.server }.getOrNull()
    val path = when {
        href.startsWith("/w/") -> href
        server != null && href.startsWith("$server/w/") -> href.removePrefix(server)
        else -> null
    }
    if (path != null) { state.pendingLink = path; return }
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(href)).addCategory(Intent.CATEGORY_BROWSABLE).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

/** HTML de l'éditeur du site affiché en natif (blocs gardés tels quels dans une description, commentaires). */
@Composable
fun HtmlBlocks(state: AppState, html: String, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    val scheme = MaterialTheme.colorScheme
    val blocks = remember(html, scheme) {
        ReaderParser.parse(html, ReaderColors(scheme.primary, scheme.surfaceContainerHighest, Color(0x66FFD54F))) { openLink(ctx, state, it) }
    }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(6.dp)) { blocks.forEach { BlockView(state, it, "") } }
}

@Composable
private fun BlockView(state: AppState, b: Block, pageId: String) {
    val ctx = LocalContext.current
    val scheme = MaterialTheme.colorScheme
    when (b) {
        is Heading -> Text(b.text, style = when (b.level) {
            1 -> MaterialTheme.typography.headlineMedium
            2 -> MaterialTheme.typography.headlineSmall
            3 -> MaterialTheme.typography.titleLarge
            else -> MaterialTheme.typography.titleMedium
        }, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 8.dp))
        is Para -> Text(b.text, style = MaterialTheme.typography.bodyLarge)
        is ListBlock -> Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            b.items.forEachIndexed { i, e ->
                Row(verticalAlignment = Alignment.Top) {
                    when {
                        e.checked != null -> Icon(if (e.checked) Icons.Filled.CheckBox else Icons.Outlined.CheckBoxOutlineBlank, null, Modifier.size(22.dp), tint = if (e.checked) scheme.primary else scheme.onSurfaceVariant)
                        b.ordered -> Text("${i + 1}.", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.width(26.dp))
                        else -> Text("•", style = MaterialTheme.typography.bodyLarge, modifier = Modifier.width(20.dp))
                    }
                    Spacer(Modifier.width(6.dp))
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) { e.blocks.forEach { BlockView(state, it, pageId) } }
                }
            }
        }
        is Quote -> {
            val accent = when (b.kind) { "warning" -> Color(0xFFEDA100); "success" -> Color(0xFF1BAF7A); "tip" -> Color(0xFF2A78D6); "info" -> scheme.primary; else -> scheme.outline }
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min).clip(RoundedCornerShape(8.dp)).background(if (b.kind != null) accent.copy(alpha = 0.10f) else Color.Transparent)) {
                Box(Modifier.width(4.dp).fillMaxHeight().background(accent))
                Column(Modifier.padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) { b.blocks.forEach { BlockView(state, it, pageId) } }
            }
        }
        is Code -> Surface(color = scheme.surfaceContainerHighest, shape = RoundedCornerShape(8.dp), modifier = Modifier.fillMaxWidth()) {
            Text(b.text, fontFamily = FontFamily.Monospace, fontSize = 13.sp, modifier = Modifier.horizontalScroll(rememberScrollState()).padding(12.dp), softWrap = false)
        }
        Rule -> HorizontalDivider(Modifier.padding(vertical = 4.dp))
        is Img -> RemoteImage(state, b)
        is Table -> Column(Modifier.horizontalScroll(rememberScrollState()).border(BorderStroke(1.dp, scheme.outlineVariant), RoundedCornerShape(8.dp)).clip(RoundedCornerShape(8.dp))) {
            b.rows.forEach { row ->
                Row(Modifier.height(IntrinsicSize.Min)) {
                    row.forEach { c ->
                        Box(Modifier.width(150.dp).fillMaxHeight().border(BorderStroke(0.5.dp, scheme.outlineVariant)).background(if (c.header) scheme.surfaceContainerHigh else Color.Transparent).padding(8.dp)) {
                            Text(c.text, style = MaterialTheme.typography.bodyMedium, fontWeight = if (c.header) FontWeight.SemiBold else null)
                        }
                    }
                }
            }
        }
        is MathBlock -> Surface(color = scheme.surfaceContainerHigh, shape = RoundedCornerShape(8.dp), modifier = Modifier.fillMaxWidth()) {
            Text(b.latex, fontFamily = FontFamily.Monospace, fontStyle = FontStyle.Italic, modifier = Modifier.horizontalScroll(rememberScrollState()).padding(12.dp), softWrap = false)
        }
        is Special -> Surface(color = scheme.surfaceContainerHigh, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth().clickable(enabled = pageId.isNotEmpty()) { state.ws?.let { openWeb(ctx, Paths.doc(it, pageId)) } }) {
            Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(when (b.kind) { "mermaid" -> Icons.Outlined.AccountTree; "excalidraw" -> Icons.Outlined.Draw; "pdf" -> Icons.Outlined.PictureAsPdf; else -> Icons.Outlined.Widgets }, null, tint = scheme.primary)
                Spacer(Modifier.width(12.dp))
                Column {
                    Text(stringResource(when (b.kind) { "mermaid" -> R.string.reader_diagram; "excalidraw" -> R.string.reader_drawing; "pdf" -> R.string.reader_pdf; else -> R.string.reader_embed }), style = MaterialTheme.typography.titleSmall)
                    Text(stringResource(R.string.reader_in_editor), style = MaterialTheme.typography.bodySmall, color = scheme.onSurfaceVariant)
                }
            }
        }
        is Toggle -> {
            var open by remember { mutableStateOf(true) }
            Column {
                Row(Modifier.fillMaxWidth().clickable { open = !open }, verticalAlignment = Alignment.CenterVertically) {
                    Icon(if (open) Icons.Filled.ExpandMore else Icons.Filled.ChevronRight, null)
                    Text(b.summary, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium)
                }
                if (open) Column(Modifier.padding(start = 24.dp, top = 6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) { b.blocks.forEach { BlockView(state, it, pageId) } }
            }
        }
    }
}

/** Image du document : seulement celles du serveur (envoyées avec la session), jamais d'un autre site. */
@Composable
private fun RemoteImage(state: AppState, img: Img) {
    val bitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(null, img.src) {
        val server = runCatching { state.api.server }.getOrNull() ?: return@produceState
        val path = when {
            img.src.startsWith("/") && !img.src.startsWith("//") -> img.src
            img.src.startsWith("$server/") -> img.src.removePrefix(server)
            else -> return@produceState
        }
        value = runCatching {
            // Images d'un document : gardées sur l'appareil (leur nom ne change jamais) pour la lecture hors ligne.
            val bytes = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { state.cache.getBytes("img-${path}") }
                ?: state.api.bytes(path).also { b -> kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { state.cache.putBytes("img-${path}", b) } }
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            var sample = 1
            while (bounds.outWidth / sample > 1800) sample *= 2
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })?.asImageBitmap()
        }.getOrNull()
    }
    bitmap?.let { Image(it, img.alt, Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)), contentScale = ContentScale.FillWidth) }
        ?: Surface(color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = RoundedCornerShape(8.dp), modifier = Modifier.fillMaxWidth()) {
            Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Outlined.Image, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.width(8.dp))
                Text(img.alt.ifBlank { "…" }, color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodySmall)
            }
        }
}
