package io.github.untypequicode.looty.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/** Mentions choisies dans un commentaire : « @Nom » devient une mention, et le reste à la modification. */
class HtmlMentionTest {
    private val design = Html.Mention("g1", "Équipe Design")
    private val ana = Html.Mention("u1", "Ana")

    @Test fun mentionsBecomeSpans() {
        assertEquals(
            "<p>Hi <span data-type=\"mention\" class=\"mention\" data-id=\"g1\" data-label=\"Équipe Design\">@Équipe Design</span> and <span data-type=\"mention\" class=\"mention\" data-id=\"u1\" data-label=\"Ana\">@Ana</span>!</p>",
            Html.fromPlain("Hi @Équipe Design and @Ana!", listOf(ana, design)),
        )
    }

    @Test fun onlyWholeNamesAfterASpace() {
        // « @Anabelle » n'est pas « @Ana » ; un e-mail n'est pas une mention.
        assertEquals("<p>@Anabelle mail ana@Ana.fr</p>", Html.fromPlain("@Anabelle mail ana@Ana.fr", listOf(ana)))
    }

    @Test fun editKeepsMentions() {
        val html = Html.fromPlain("Hi @Équipe Design", listOf(design))
        assertEquals(listOf(design), Html.mentionsOf(html))
        assertEquals(html, Html.fromPlain(Html.toPlain(html), Html.mentionsOf(html)))
    }
}
