package io.github.untypequicode.looty.ui

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** L'éditeur natif de description relit et réécrit l'HTML du site sans rien perdre. */
class RichDocTest {
    private fun round(html: String) = RichDoc.toHtml(RichDoc.parse(html))

    @Test fun inlineFormatting() {
        val html = "<p>Hello <strong>bold</strong>, <em>it</em>, <u>u</u>, <s>s</s>, <code>x*y</code> and <a href=\"https://x.io/a(b)\">a <strong>link</strong></a></p>"
        assertEquals("<p>Hello <strong>bold</strong>, <em>it</em>, <u>u</u>, <s>s</s>, <code>x*y</code> and <a href=\"https://x.io/a%28b%29\">a <strong>link</strong></a></p>", round(html))
    }

    @Test fun literalMarkersStayText() {
        val html = "<p>2 * 3 * 4 = 24, snake__case, ~~no~~ [x](y) \\ end</p>"
        val blocks = RichDoc.parse(html)
        assertEquals("<p>2 * 3 * 4 = 24, snake__case, ~~no~~ [x](y) \\ end</p>", RichDoc.toHtml(blocks))
    }

    @Test fun taskListFromTipTap() {
        val html = "<ul data-type=\"taskList\"><li data-checked=\"true\" data-type=\"taskItem\"><label><input type=\"checkbox\" checked=\"checked\"><span></span></label><div><p>Done</p></div></li>" +
            "<li data-checked=\"false\" data-type=\"taskItem\"><label><input type=\"checkbox\"><span></span></label><div><p>Todo</p></div></li></ul>"
        val blocks = RichDoc.parse(html)
        assertEquals(listOf(BType.CHECK, BType.CHECK, BType.P), blocks.map { it.type })
        assertEquals(listOf(true, false), blocks.take(2).map { it.checked })
        assertEquals("<ul data-type=\"taskList\"><li data-type=\"taskItem\" data-checked=\"true\"><p>Done</p></li><li data-type=\"taskItem\" data-checked=\"false\"><p>Todo</p></li></ul>", RichDoc.toHtml(blocks))
    }

    @Test fun nestedLists() {
        val html = "<ul><li><p>a</p><ol><li><p>a1</p></li><li><p>a2</p></li></ol></li><li><p>b</p></li></ul><p>after</p>"
        val blocks = RichDoc.parse(html)
        assertEquals(listOf(0, 1, 1, 0, 0), blocks.map { it.indent })
        assertEquals(html, RichDoc.toHtml(blocks))
    }

    @Test fun unsupportedBlocksKeptAsIs() {
        val table = "<table><tbody><tr><td><p>x</p></td></tr></tbody></table>"
        val mention = "<p>Hi <span class=\"mention\" data-type=\"mention\" data-id=\"u1\" data-label=\"Maya\">@Maya</span></p>"
        val html = "<p>before</p>$table$mention<p>after</p>"
        val blocks = RichDoc.parse(html)
        assertEquals(listOf(BType.P, BType.RAW, BType.RAW, BType.P), blocks.map { it.type })
        val out = RichDoc.toHtml(blocks)!!
        assertTrue(out, out.contains("data-id=\"u1\"") && out.contains("<td>"))
        assertEquals(out, round(out))
    }

    @Test fun headingsQuotesCode() {
        val html = "<h2>Title</h2><blockquote><p>q1</p><p>q2</p></blockquote><pre><code class=\"language-kotlin\">val a = 1 &lt; 2\n  *x*</code></pre>"
        assertEquals(html, round(html))
    }

    @Test fun emptyDescription() {
        assertEquals(null, round(""))
        assertEquals(null, round("<p></p>"))
        assertEquals(null, RichDoc.toHtml(listOf(RichDoc.block(BType.P))))
    }

    @Test fun markupToHtml() {
        assertEquals("<strong>a <em>b</em></strong>", RichDoc.inlineHtml("**a *b***"))
        assertEquals("**open", RichDoc.inlineHtml("**open"))
        assertEquals("a<br>b &amp; &lt;c&gt;", RichDoc.inlineHtml("a\nb & <c>"))
        assertEquals("<a href=\"https://looty.app\">site</a>", RichDoc.inlineHtml("[site](looty.app)"))
        assertEquals("<a href=\"https://javascript:alert(1\">x</a>)", RichDoc.inlineHtml("[x](javascript:alert(1))"))
    }

    @Test fun enterSplitsAndEndsLists() {
        val st = DescEditorState("<ul><li><p>one</p></li></ul>") {}
        val id = st.blocks[0].id
        // Entrée à la fin de « one » : nouvel élément de liste.
        st.onText(id, TextFieldValue("one\n", TextRange(4)))
        assertEquals(listOf(BType.BULLET, BType.BULLET, BType.P), st.blocks.map { it.type })
        // Entrée sur l'élément vide : fin de la liste.
        val second = st.blocks[1].id
        st.onText(second, TextFieldValue("\n", TextRange(1)))
        assertEquals(listOf(BType.BULLET, BType.P, BType.P), st.blocks.map { it.type })
        assertEquals("<ul><li><p>one</p></li></ul>", RichDoc.toHtml(st.blocks))
    }

    @Test fun shortcutsAndBackspace() {
        val st = DescEditorState(null) {}
        val id = st.blocks[0].id
        st.onText(id, TextFieldValue("[] ", TextRange(3)))
        assertEquals(BType.CHECK, st.blocks[0].type)
        st.onText(id, TextFieldValue("buy milk", TextRange(8)))
        st.toggleCheck(id)
        assertEquals("<ul data-type=\"taskList\"><li data-type=\"taskItem\" data-checked=\"true\"><p>buy milk</p></li></ul>", RichDoc.toHtml(st.blocks))
        // Effacement au début : redevient un paragraphe, puis fusionne avec le précédent.
        assertTrue(st.backspaceAtStart(id))
        assertEquals(BType.P, st.blocks[0].type)
        st.onText(id, TextFieldValue("buy milk\nand eggs", TextRange(9)))
        assertEquals(2, st.blocks.count { it.text.text.isNotEmpty() })
        assertTrue(st.backspaceAtStart(st.blocks[1].id))
        assertEquals("buy milkand eggs", st.blocks[0].text.text)
    }

    @Test fun wrapSelection() {
        val st = DescEditorState("<p>hello world</p>") {}
        val id = st.blocks[0].id
        st.focusedId = id
        st.onText(id, TextFieldValue("hello world", TextRange(6, 11)))
        st.wrap("**")
        assertEquals("<p>hello <strong>world</strong></p>", RichDoc.toHtml(st.blocks))
        st.wrap("**")
        assertEquals("<p>hello world</p>", RichDoc.toHtml(st.blocks))
    }
}
