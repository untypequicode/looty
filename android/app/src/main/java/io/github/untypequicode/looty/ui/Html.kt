package io.github.untypequicode.looty.ui

/**
 * Descriptions et commentaires sont stockés en HTML (éditeur du site). L'application les
 * affiche mis en forme ; une description n'est modifiée en texte brut que si elle est
 * simple (paragraphes, gras, italique, liens), pour ne rien perdre.
 */
object Html {
    private fun esc(s: String) = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

    /** Texte saisi → HTML (un paragraphe par ligne, liens rendus cliquables). */
    fun fromPlain(text: String): String = text.trim().split(Regex("\n{2,}")).filter { it.isNotBlank() }.joinToString("") { para ->
        val linked = esc(para.trim()).replace(Regex("https?://[^\\s<]+")) { "<a href=\"${it.value}\">${it.value}</a>" }
        "<p>${linked.replace("\n", "<br>")}</p>"
    }

    private val simpleTags = setOf("p", "br", "strong", "b", "em", "i", "a", "span")

    fun isSimple(html: String?): Boolean {
        if (html.isNullOrBlank()) return true
        return Regex("<\\s*/?\\s*([a-zA-Z0-9]+)").findAll(html).all { it.groupValues[1].lowercase() in simpleTags }
    }

    /** HTML simple → texte (pour l'éditer). */
    fun toPlain(html: String?): String {
        if (html.isNullOrBlank()) return ""
        return html.replace(Regex("<br\\s*/?>", RegexOption.IGNORE_CASE), "\n")
            .replace(Regex("</p>\\s*<p[^>]*>", RegexOption.IGNORE_CASE), "\n\n")
            .replace(Regex("<[^>]+>"), "")
            .replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ").replace("&amp;", "&")
            .trim()
    }
}
