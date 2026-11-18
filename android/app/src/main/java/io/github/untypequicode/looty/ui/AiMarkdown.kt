package io.github.untypequicode.looty.ui

/** Inline d'une réponse de l'assistant (Markdown + formules). */
sealed interface MdInline {
    data class Text(val s: String) : MdInline
    data class Bold(val c: List<MdInline>) : MdInline
    data class Italic(val c: List<MdInline>) : MdInline
    data class Code(val s: String) : MdInline
    data class Link(val c: List<MdInline>, val url: String) : MdInline
    data class Math(val tex: String, val display: Boolean) : MdInline
}

data class MdListItem(val depth: Int, val marker: String, val inlines: List<MdInline>)

sealed interface MdBlock {
    data class Para(val inlines: List<MdInline>) : MdBlock
    data class Heading(val level: Int, val inlines: List<MdInline>) : MdBlock
    data class Code(val lang: String, val text: String) : MdBlock
    data class Quote(val inlines: List<MdInline>) : MdBlock
    data class Items(val items: List<MdListItem>) : MdBlock
    data class DisplayMath(val tex: String) : MdBlock
    data object Rule : MdBlock
}

/** Liens : seuls http(s) et mailto (jamais `javascript:`), comme le site. */
fun safeUrl(u: String): Boolean = Regex("^(https?:|mailto:)", RegexOption.IGNORE_CASE).containsMatchIn(u.trim())

private fun isPunct(c: Char) = !c.isLetterOrDigit() && !c.isWhitespace()

/**
 * Markdown inline : **gras**, *italique*, `code`, [lien](url) et formules ($..$, $$..$$, \(..\), \[..\]).
 * Une formule non fermée reste du texte (pendant le flux).
 */
fun parseInline(s: String): List<MdInline> {
    val out = mutableListOf<MdInline>()
    val sb = StringBuilder()
    fun flush() { if (sb.isNotEmpty()) { out.add(MdInline.Text(sb.toString())); sb.setLength(0) } }
    val n = s.length
    var i = 0
    while (i < n) {
        val c = s[i]
        if (c == '\\' && i + 1 < n) {
            val nx = s[i + 1]
            if (nx == '(' || nx == '[') {
                val closer = if (nx == '(') "\\)" else "\\]"
                val end = s.indexOf(closer, i + 2)
                if (end > i + 2 && (nx == '[' || !s.substring(i + 2, end).contains("\n\n"))) {
                    flush(); out.add(MdInline.Math(s.substring(i + 2, end).trim(), nx == '[')); i = end + 2; continue
                }
            }
            if (isPunct(nx)) { sb.append(nx); i += 2; continue }
            sb.append(c); i++; continue
        }
        if (c == '`') {
            var k = i
            while (k < n && s[k] == '`') k++
            val ticks = s.substring(i, k)
            val close = s.indexOf(ticks, k)
            if (close > k) { flush(); out.add(MdInline.Code(s.substring(k, close).trim())); i = close + ticks.length; continue }
            sb.append(ticks); i = k; continue
        }
        if (c == '$') {
            if (i + 1 < n && s[i + 1] == '$') {
                val end = s.indexOf("$$", i + 2)
                if (end > i + 2) { flush(); out.add(MdInline.Math(s.substring(i + 2, end).trim(), true)); i = end + 2; continue }
            } else if (i + 1 < n && !s[i + 1].isWhitespace()) {
                var j = i + 1
                var found = -1
                while (j < n) {
                    if (s[j] == '\n' && j + 1 < n && s[j + 1] == '\n') break
                    if (s[j] == '\\') { j += 2; continue }
                    if (s[j] == '$') { found = j; break }
                    j++
                }
                // « 5 $ et 6 $ » n'est pas une formule : fermeture collée au contenu et non suivie d'un chiffre.
                if (found > i + 1 && !s[found - 1].isWhitespace() && (found + 1 >= n || !s[found + 1].isDigit())) {
                    flush(); out.add(MdInline.Math(s.substring(i + 1, found), false)); i = found + 1; continue
                }
            }
            sb.append(c); i++; continue
        }
        if (c == '*' || c == '_') {
            val double = i + 1 < n && s[i + 1] == c
            val mark = if (double) "$c$c" else "$c"
            val prevWord = i > 0 && s[i - 1].isLetterOrDigit()
            if (!(c == '_' && prevWord) && i + mark.length < n && !s[i + mark.length].isWhitespace()) {
                var end = s.indexOf(mark, i + mark.length)
                // `_` dans un mot (snake_case) : on cherche une fermeture en fin de mot.
                while (end > 0 && c == '_' && end + mark.length < n && s[end + mark.length].isLetterOrDigit()) end = s.indexOf(mark, end + 1)
                if (end > i + mark.length && !s[end - 1].isWhitespace()) {
                    val inner = parseInline(s.substring(i + mark.length, end))
                    flush(); out.add(if (double) MdInline.Bold(inner) else MdInline.Italic(inner)); i = end + mark.length; continue
                }
            }
            sb.append(c); i++; continue
        }
        if (c == '[') {
            val close = s.indexOf("](", i + 1)
            if (close > i) {
                val end = s.indexOf(')', close + 2)
                if (end > close) {
                    val url = s.substring(close + 2, end).trim()
                    val inner = parseInline(s.substring(i + 1, close))
                    flush()
                    if (safeUrl(url)) out.add(MdInline.Link(inner, url)) else out.addAll(inner)
                    i = end + 1; continue
                }
            }
        }
        sb.append(c); i++
    }
    flush()
    return out
}

private val heading = Regex("^(#{1,6})\\s+(.*)$")
private val rule = Regex("^(-{3,}|\\*{3,}|_{3,})$")
private val item = Regex("^(\\s*)([-*+]|\\d+[.)])\\s+(.*)$")

/** Paragraphe → paragraphes et formules en bloc (une formule `$$..$$` au milieu d'un texte). */
private fun paragraph(text: String, out: MutableList<MdBlock>) {
    val run = mutableListOf<MdInline>()
    fun flush() {
        if (run.isNotEmpty() && run.any { !(it is MdInline.Text && it.s.isBlank()) }) out.add(MdBlock.Para(run.toList()))
        run.clear()
    }
    for (x in parseInline(text)) {
        if (x is MdInline.Math && x.display) { flush(); out.add(MdBlock.DisplayMath(x.tex)) } else run.add(x)
    }
    flush()
}

/** Réponse Markdown → blocs. Les blocs ouverts (code, formule) restent affichables pendant le flux. */
fun parseMarkdown(src: String): List<MdBlock> {
    val out = mutableListOf<MdBlock>()
    val lines = src.replace("\r\n", "\n").split("\n")
    val para = StringBuilder()
    fun flushPara() { if (para.isNotBlank()) paragraph(para.toString(), out); para.setLength(0) }
    var i = 0
    while (i < lines.size) {
        val line = lines[i]
        val t = line.trim()
        if (t.startsWith("```") || t.startsWith("~~~")) {
            flushPara()
            val fence = t.take(3)
            val lang = t.drop(3).trim().substringBefore(' ').lowercase()
            val body = mutableListOf<String>()
            i++
            while (i < lines.size && !lines[i].trim().startsWith(fence)) { body.add(lines[i]); i++ }
            i++ // clôture
            out.add(if (lang == "math") MdBlock.DisplayMath(body.joinToString("\n").trim()) else MdBlock.Code(lang, body.joinToString("\n")))
            continue
        }
        if (t.startsWith("\$\$") || t.startsWith("\\[")) {
            val opener = if (t.startsWith("\$\$")) "\$\$" else "\\["
            val closer = if (opener == "\$\$") "\$\$" else "\\]"
            val first = t.substring(2)
            val sameLine = first.indexOf(closer)
            if (sameLine >= 0) {
                // Formule sur une ligne ; ce qui suit la fermeture reste du texte.
                val rest = first.substring(sameLine + 2).trim()
                flushPara()
                out.add(MdBlock.DisplayMath(first.substring(0, sameLine).trim()))
                if (rest.isNotEmpty()) para.append(rest)
                i++; continue
            }
            var j = i + 1
            val body = mutableListOf(first)
            var closed = false
            while (j < lines.size) {
                val k = lines[j].indexOf(closer)
                if (k >= 0) { body.add(lines[j].substring(0, k)); closed = true; break }
                body.add(lines[j]); j++
            }
            if (closed) {
                flushPara()
                out.add(MdBlock.DisplayMath(body.joinToString("\n").trim()))
                val rest = lines[j].substringAfter(closer).trim()
                if (rest.isNotEmpty()) para.append(rest)
                i = j + 1; continue
            }
            // Non fermée (flux en cours) : reste du texte.
        }
        if (t.isEmpty()) { flushPara(); i++; continue }
        val h = heading.matchEntire(t)
        if (h != null) { flushPara(); out.add(MdBlock.Heading(h.groupValues[1].length, parseInline(h.groupValues[2]))); i++; continue }
        if (rule.matches(t)) { flushPara(); out.add(MdBlock.Rule); i++; continue }
        if (t.startsWith("|")) {
            flushPara()
            val rows = mutableListOf<String>()
            while (i < lines.size && lines[i].trim().startsWith("|")) { rows.add(lines[i].trim()); i++ }
            out.add(MdBlock.Code("table", rows.joinToString("\n")))
            continue
        }
        if (t.startsWith(">")) {
            flushPara()
            val q = mutableListOf<String>()
            while (i < lines.size && lines[i].trim().startsWith(">")) { q.add(lines[i].trim().removePrefix(">").removePrefix(" ")); i++ }
            out.add(MdBlock.Quote(parseInline(q.joinToString("\n"))))
            continue
        }
        val m = item.matchEntire(line)
        if (m != null) {
            flushPara()
            val items = mutableListOf<MdListItem>()
            var k = i
            while (k < lines.size) {
                val mm = item.matchEntire(lines[k]) ?: break
                val marker = mm.groupValues[2].let { if (it[0].isDigit()) it else "•" }
                items.add(MdListItem(mm.groupValues[1].length / 2, marker, parseInline(mm.groupValues[3])))
                k++
            }
            out.add(MdBlock.Items(items))
            i = k; continue
        }
        if (para.isNotEmpty()) para.append('\n')
        para.append(line.trimEnd())
        i++
    }
    flushPara()
    return out
}
