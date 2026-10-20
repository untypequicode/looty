package io.github.untypequicode.looty.ui

/**
 * Descriptions et commentaires sont stockés en HTML (éditeur du site). L'application les
 * affiche mis en forme ; une description n'est modifiée en texte brut que si elle est
 * simple (paragraphes, gras, italique, liens), pour ne rien perdre.
 */
object Html {
    private fun esc(s: String) = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

    /** Texte saisi → HTML (un paragraphe par ligne, liens rendus cliquables). */
    fun fromPlain(text: String, mentions: List<Mention> = emptyList()): String = text.trim().split(Regex("\n{2,}")).filter { it.isNotBlank() }.joinToString("") { para ->
        var linked = esc(para.trim()).replace(Regex("https?://[^\\s<]+")) { "<a href=\"${it.value}\">${it.value}</a>" }
        // Mentions choisies dans la liste : « @Nom » devient une mention (notifiée par le serveur).
        for (m in mentions.sortedByDescending { it.label.length }) {
            val label = esc(m.label)
            val attr = label.replace("\"", "&quot;")
            linked = linked.replace(Regex("(^|(?<=[\\s(]))@" + Regex.escape(label) + "(?![\\p{L}\\p{N}])")) {
                "<span data-type=\"mention\" class=\"mention\" data-id=\"${m.id}\" data-label=\"$attr\">@$label</span>"
            }
        }
        "<p>${linked.replace("\n", "<br>")}</p>"
    }

    /** Personne ou groupe mentionné dans un champ de texte (« @Nom »). */
    data class Mention(val id: String, val label: String)

    /** Mentions d'un HTML existant (pour les garder quand on modifie le texte). */
    fun mentionsOf(html: String?): List<Mention> = Regex("<span[^>]*data-type=\"mention\"[^>]*>").findAll(html.orEmpty()).mapNotNull { m ->
        val tag = m.value
        val id = Regex("data-id=\"([^\"]+)\"").find(tag)?.groupValues?.get(1) ?: return@mapNotNull null
        val label = Regex("data-label=\"([^\"]*)\"").find(tag)?.groupValues?.get(1) ?: return@mapNotNull null
        Mention(id, label.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
    }.toList()

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
