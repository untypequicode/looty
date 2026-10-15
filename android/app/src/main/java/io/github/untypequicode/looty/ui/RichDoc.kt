package io.github.untypequicode.looty.ui

import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.graphics.Color
import org.jsoup.Jsoup
import org.jsoup.nodes.Element
import org.jsoup.nodes.Node
import org.jsoup.nodes.TextNode
import java.util.concurrent.atomic.AtomicLong

/** Type d'un bloc de la description. RAW : contenu que l'application ne sait pas modifier (gardé tel quel). */
enum class BType { P, H1, H2, H3, BULLET, NUMBER, CHECK, QUOTE, CODE, RAW }

/**
 * Bloc de l'éditeur natif. Le texte porte la mise en forme en balisage léger :
 * **gras**, *italique*, __souligné__, ~~barré~~, `code`, [lien](adresse) ; `\` protège un caractère.
 * [html] : HTML d'origine d'un bloc RAW, ou langage d'un bloc de code.
 */
data class EBlock(
    val id: Long,
    val type: BType,
    val text: TextFieldValue = TextFieldValue(""),
    val indent: Int = 0,
    val checked: Boolean = false,
    val html: String = "",
) {
    val isList get() = type == BType.BULLET || type == BType.NUMBER || type == BType.CHECK
}

/**
 * Conversion entre l'HTML de l'éditeur du site (TipTap) et les blocs de l'éditeur natif, dans les deux
 * sens, sans perte : ce que l'application ne sait pas modifier (tableaux, images, mentions…) reste en bloc RAW.
 */
object RichDoc {
    private val seq = AtomicLong()
    fun newId() = seq.incrementAndGet()
    fun block(type: BType, text: String = "", indent: Int = 0, checked: Boolean = false, html: String = "") =
        EBlock(newId(), type, TextFieldValue(text, TextRange(text.length)), indent, checked, html)

    private class Unsupported : Exception()

    // ---------------------------------------------------------------- HTML → blocs

    fun parse(html: String?): List<EBlock> {
        val out = mutableListOf<EBlock>()
        if (!html.isNullOrBlank()) {
            val body = Jsoup.parseBodyFragment(html).body()
            val stray = mutableListOf<Node>()
            fun flush() {
                if (stray.isEmpty()) return
                out += try { block(BType.P, inline(stray)) } catch (_: Unsupported) { block(BType.RAW, html = stray.joinToString("") { it.outerHtml() }) }
                stray.clear()
            }
            for (n in body.childNodes()) {
                if (n is TextNode) { if (!n.isBlank) stray.add(n); continue }
                val el = n as? Element ?: continue
                if (el.tagName() in INLINE_TAGS) { stray.add(el); continue }
                flush()
                element(el, out)
            }
            flush()
        }
        // Toujours un endroit où écrire à la fin.
        if (out.isEmpty() || out.last().type != BType.P) out += block(BType.P)
        return out
    }

    private val INLINE_TAGS = setOf("a", "strong", "b", "em", "i", "u", "s", "del", "strike", "code", "span", "br", "mark")

    private fun raw(el: Element) = block(BType.RAW, html = el.outerHtml())

    private fun element(el: Element, out: MutableList<EBlock>) {
        try {
            when (val tag = el.tagName()) {
                "p" -> out += block(BType.P, inline(el.childNodes()))
                "h1" -> out += block(BType.H1, inline(el.childNodes()))
                "h2" -> out += block(BType.H2, inline(el.childNodes()))
                "h3", "h4", "h5", "h6" -> out += block(BType.H3, inline(el.childNodes()))
                "ul", "ol" -> { val tmp = mutableListOf<EBlock>(); list(el, 0, tmp); out += tmp }
                "blockquote" -> {
                    if (el.hasAttr("data-notice-type") || el.children().any { it.tagName() != "p" }) throw Unsupported()
                    out += el.children().map { block(BType.QUOTE, inline(it.childNodes())) }
                }
                "pre" -> {
                    val code = el.selectFirst("code")
                    val lang = code?.classNames()?.firstOrNull { it.startsWith("language-") }?.removePrefix("language-").orEmpty()
                    out += block(BType.CODE, (code ?: el).wholeText().trimEnd('\n'), html = lang)
                }
                else -> if (tag == "div" && el.attributes().isEmpty) el.children().forEach { element(it, out) } else out += raw(el)
            }
        } catch (_: Unsupported) {
            out += raw(el)
        }
    }

    private fun list(el: Element, indent: Int, out: MutableList<EBlock>) {
        val type = when {
            el.attr("data-type") == "taskList" -> BType.CHECK
            el.tagName() == "ol" -> BType.NUMBER
            else -> BType.BULLET
        }
        for (li in el.children()) {
            if (li.tagName() != "li") throw Unsupported()
            // Élément de liste de tâches : <label> (case) puis <div> (contenu).
            val parts = li.children().filter { it.tagName() != "label" }.flatMap { if (it.tagName() == "div") it.children() else listOf(it) }
            val paras = parts.filter { it.tagName() == "p" }
            val nested = parts.filter { it.tagName() == "ul" || it.tagName() == "ol" }
            if (paras.size > 1 || parts.size != paras.size + nested.size) throw Unsupported()
            val text = paras.firstOrNull()?.let { inline(it.childNodes()) }
                ?: inline(li.childNodes().filter { it is TextNode || (it is Element && it.tagName() in INLINE_TAGS) })
            out += block(type, text, indent, checked = li.attr("data-checked") == "true")
            nested.forEach { list(it, indent + 1, out) }
        }
    }

    private const val SPECIAL = "\\*_~`[]"

    /** Texte brut → balisage (les caractères de mise en forme sont protégés). */
    fun escape(s: String): String = buildString {
        var i = 0
        while (i < s.length) {
            val c = s[i]
            val next = s.getOrNull(i + 1)
            when {
                c == '*' || c == '`' || c == '[' -> append('\\').append(c)
                c == '\\' && next != null && next in SPECIAL -> append("\\\\")
                (c == '_' || c == '~') && (next == c || s.getOrNull(i - 1) == c) -> append('\\').append(c)
                else -> append(c)
            }
            i++
        }
    }

    private fun inline(nodes: List<Node>): String = buildString { nodes.forEach { node(it) } }

    private fun StringBuilder.node(n: Node) {
        if (n is TextNode) { append(escape(n.wholeText.replace('\n', ' '))); return }
        val el = n as? Element ?: return
        fun wrap(m: String) { append(m); el.childNodes().forEach { node(it) }; append(m) }
        when (el.tagName()) {
            "br" -> append('\n')
            "strong", "b" -> wrap("**")
            "em", "i" -> wrap("*")
            "u" -> wrap("__")
            "s", "del", "strike" -> wrap("~~")
            "code" -> { val t = el.wholeText(); if ('`' in t) throw Unsupported(); append('`').append(t).append('`') }
            "a" -> {
                val href = el.attr("href")
                if (el.hasAttr("data-type") || href.isBlank() || el.attributes().any { it.key !in setOf("href", "target", "rel", "class") }) throw Unsupported()
                append('['); el.childNodes().forEach { node(it) }
                append("](").append(href.replace(" ", "%20").replace(")", "%29").replace("(", "%28")).append(')')
            }
            "span" -> if (el.attributes().isEmpty) el.childNodes().forEach { node(it) } else throw Unsupported()
            else -> throw Unsupported()
        }
    }

    // ---------------------------------------------------------------- balisage → jetons

    private sealed interface Tok
    private data class Lit(val s: Int, val e: Int) : Tok
    private data class Esc(val at: Int) : Tok
    private data class Mark(val tag: String, val s: Int, val e: Int, val open: Boolean) : Tok
    private data class CodeT(val s: Int, val e: Int) : Tok
    private data class LinkT(val s: Int, val label: List<Tok>, val urlS: Int, val urlE: Int, val e: Int) : Tok
    private data class Br(val at: Int) : Tok

    private val MARKS = listOf("**" to "strong", "__" to "u", "~~" to "s", "*" to "em")

    private fun tokens(s: String, from: Int = 0, to: Int = s.length): List<Tok> {
        val out = mutableListOf<Tok>()
        val open = mutableListOf<String>()
        var i = from
        var lit = -1
        fun flushLit() { if (lit >= 0) { out += Lit(lit, i); lit = -1 } }
        while (i < to) {
            val c = s[i]
            if (c == '\\' && i + 1 < to && s[i + 1] in SPECIAL) { flushLit(); out += Esc(i); i += 2; continue }
            if (c == '\n') { flushLit(); out += Br(i); i++; continue }
            if (c == '`') {
                val end = s.indexOf('`', i + 1)
                if (end in (i + 2) until to) { flushLit(); out += CodeT(i, end + 1); i = end + 1; continue }
            }
            if (c == '[') {
                val close = closingBracket(s, i, to)
                if (close > 0 && close + 1 < to && s[close + 1] == '(') {
                    val par = s.indexOf(')', close + 2)
                    if (par in (close + 3) until to && s.substring(close + 2, par).none { it.isWhitespace() }) {
                        flushLit(); out += LinkT(i, tokens(s, i + 1, close), close + 2, par, par + 1); i = par + 1; continue
                    }
                }
            }
            // Le marqueur ouvert le plus récent se referme en premier (« ***» après « **a *b »).
            val top = open.lastOrNull()?.let { t -> MARKS.first { it.second == t } }
            if (top != null && s.startsWith(top.first, i)) { flushLit(); out += Mark(top.second, i, i + top.first.length, false); open.removeAt(open.size - 1); i += top.first.length; continue }
            val mark = MARKS.firstOrNull { s.startsWith(it.first, i) }
            if (mark != null) {
                val (m, tag) = mark
                val len = m.length
                if (tag in open) { flushLit(); out += Mark(tag, i, i + len, false); open.remove(tag); i += len; continue }
                val again = s.indexOf(m, i + len)
                // Ouvre seulement si le marqueur est refermé plus loin et entoure du texte.
                if (again in (i + len + 1) until to && !s[i + len].isWhitespace()) { flushLit(); out += Mark(tag, i, i + len, true); open += tag; i += len; continue }
            }
            if (lit < 0) lit = i
            i++
        }
        flushLit()
        return out
    }

    private fun closingBracket(s: String, start: Int, to: Int): Int {
        var depth = 0
        var i = start
        while (i < to) {
            when {
                s[i] == '\\' -> i++
                s[i] == '[' -> depth++
                s[i] == ']' -> { depth--; if (depth == 0) return i }
            }
            i++
        }
        return -1
    }

    // ---------------------------------------------------------------- balisage → HTML

    private fun esc(s: String) = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;")

    fun inlineHtml(s: String): String = buildString { html(s, tokens(s), this) }

    private fun html(s: String, toks: List<Tok>, sb: StringBuilder) {
        val stack = mutableListOf<String>()
        for (t in toks) when (t) {
            is Lit -> sb.append(esc(s.substring(t.s, t.e)))
            is Esc -> sb.append(esc(s[t.at + 1].toString()))
            is Br -> sb.append("<br>")
            is CodeT -> sb.append("<code>").append(esc(s.substring(t.s + 1, t.e - 1))).append("</code>")
            is LinkT -> {
                val url = s.substring(t.urlS, t.urlE)
                val safe = if (Regex("^(https?:|mailto:|tel:|/|#)", RegexOption.IGNORE_CASE).containsMatchIn(url)) url else "https://$url"
                sb.append("<a href=\"").append(esc(safe)).append("\">"); html(s, t.label, sb); sb.append("</a>")
            }
            is Mark -> if (t.open) { sb.append("<${t.tag}>"); stack += t.tag } else {
                // Marqueurs mal imbriqués : on referme ceux du dessus puis on les rouvre.
                val idx = stack.lastIndexOf(t.tag)
                if (idx >= 0) {
                    val above = stack.subList(idx + 1, stack.size).toList()
                    above.asReversed().forEach { sb.append("</$it>") }
                    sb.append("</${t.tag}>")
                    stack.removeAt(idx)
                    above.forEach { sb.append("<$it>") }
                }
            }
        }
        stack.asReversed().forEach { sb.append("</$it>") }
    }

    // ---------------------------------------------------------------- blocs → HTML

    fun toHtml(blocks: List<EBlock>): String? {
        val list = blocks.dropLastWhile { it.type == BType.P && it.text.text.isBlank() }
        if (list.all { it.type != BType.RAW && it.type != BType.CODE && it.text.text.isBlank() && !(it.isList) }) return null
        val sb = StringBuilder()
        var i = 0
        while (i < list.size) {
            val b = list[i]
            when (b.type) {
                BType.P -> sb.append("<p>").append(inlineHtml(b.text.text)).append("</p>")
                BType.H1, BType.H2, BType.H3 -> { val h = "h" + (b.type.ordinal - BType.H1.ordinal + 1); sb.append("<$h>").append(inlineHtml(b.text.text)).append("</$h>") }
                BType.CODE -> sb.append("<pre><code").append(if (b.html.isNotBlank()) " class=\"language-${esc(b.html)}\"" else "").append(">").append(esc(b.text.text)).append("</code></pre>")
                BType.RAW -> sb.append(b.html)
                BType.QUOTE -> {
                    var j = i
                    sb.append("<blockquote>")
                    while (j < list.size && list[j].type == BType.QUOTE) { sb.append("<p>").append(inlineHtml(list[j].text.text)).append("</p>"); j++ }
                    sb.append("</blockquote>")
                    i = j; continue
                }
                BType.BULLET, BType.NUMBER, BType.CHECK -> {
                    var j = i
                    while (j < list.size && list[j].isList) j++
                    // Retraits remis en ordre : le premier élément au niveau 0, chaque élément au plus un niveau plus bas que le précédent.
                    var prev = -1
                    val items = list.subList(i, j).map { it.copy(indent = it.indent.coerceIn(0, prev + 1).also { lvl -> prev = lvl }) }
                    listHtml(items, 0, sb)
                    i = j; continue
                }
            }
            i++
        }
        return sb.toString()
    }

    private fun listHtml(items: List<EBlock>, level: Int, sb: StringBuilder) {
        var k = 0
        while (k < items.size) {
            val kind = items[k].type
            sb.append(when (kind) { BType.CHECK -> "<ul data-type=\"taskList\">"; BType.NUMBER -> "<ol>"; else -> "<ul>" })
            while (k < items.size && items[k].indent == level && items[k].type == kind) {
                val it = items[k]
                sb.append(if (kind == BType.CHECK) "<li data-type=\"taskItem\" data-checked=\"${it.checked}\">" else "<li>")
                sb.append("<p>").append(inlineHtml(it.text.text)).append("</p>")
                var m = k + 1
                while (m < items.size && items[m].indent > level) m++
                if (m > k + 1) listHtml(items.subList(k + 1, m), level + 1, sb)
                sb.append("</li>")
                k = m
            }
            sb.append(if (kind == BType.NUMBER) "</ol>" else "</ul>")
        }
    }

    // ---------------------------------------------------------------- affichage du balisage

    /** Met en forme le texte pendant la saisie ; les marqueurs restent visibles, en plus discret. */
    class Markup(private val dim: Color, private val link: Color, private val codeBg: Color) : VisualTransformation {
        override fun filter(text: AnnotatedString): TransformedText {
            val s = text.text
            val out = buildAnnotatedString {
                append(s)
                style(s, tokens(s), emptySet(), this)
            }
            return TransformedText(out, OffsetMapping.Identity)
        }

        private fun spanOf(tags: Set<String>) = SpanStyle(
            fontWeight = if ("strong" in tags) FontWeight.Bold else null,
            fontStyle = if ("em" in tags) FontStyle.Italic else null,
            textDecoration = TextDecoration.combine(listOfNotNull(
                if ("u" in tags || "a" in tags) TextDecoration.Underline else null,
                if ("s" in tags) TextDecoration.LineThrough else null,
            )),
            color = if ("a" in tags) link else Color.Unspecified,
        )

        private fun style(s: String, toks: List<Tok>, base: Set<String>, b: AnnotatedString.Builder) {
            val active = base.toMutableSet()
            val dimStyle = SpanStyle(color = dim)
            for (t in toks) when (t) {
                is Lit -> if (active.isNotEmpty()) b.addStyle(spanOf(active), t.s, t.e)
                is Esc -> { b.addStyle(dimStyle, t.at, t.at + 1); if (active.isNotEmpty()) b.addStyle(spanOf(active), t.at + 1, t.at + 2) }
                is Br -> {}
                is CodeT -> {
                    b.addStyle(dimStyle, t.s, t.s + 1); b.addStyle(dimStyle, t.e - 1, t.e)
                    b.addStyle(SpanStyle(fontFamily = FontFamily.Monospace, background = codeBg), t.s + 1, t.e - 1)
                }
                is LinkT -> {
                    b.addStyle(dimStyle, t.s, t.s + 1)
                    style(s, t.label, active + "a", b)
                    b.addStyle(dimStyle, t.urlS - 2, t.e)
                }
                is Mark -> { b.addStyle(dimStyle, t.s, t.e); if (t.open) active += t.tag else active -= t.tag }
            }
        }
    }
}
