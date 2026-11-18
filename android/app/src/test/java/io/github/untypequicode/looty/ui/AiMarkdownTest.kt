package io.github.untypequicode.looty.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AiMarkdownTest {
    @Test fun inlineMathBetweenText() {
        assertEquals(listOf(MdInline.Text("Soit "), MdInline.Math("x^2", false), MdInline.Text(" ici")), parseInline("Soit \$x^2\$ ici"))
    }

    @Test fun moneyIsNotMath() {
        assertEquals(listOf(MdInline.Text("5 \$ et 6 \$")), parseInline("5 \$ et 6 \$"))
        assertEquals(listOf(MdInline.Text("coûte \$5 puis \$6")), parseInline("coûte \$5 puis \$6"))
    }

    @Test fun unclosedMathStaysText() {
        assertEquals(listOf(MdInline.Text("a \$x^2")), parseInline("a \$x^2"))
    }

    @Test fun parenthesisAndBracketDelimiters() {
        assertEquals(listOf(MdInline.Math("a+b", false)), parseInline("\\(a+b\\)"))
        assertEquals(listOf(MdInline.Math("a+b", true)), parseInline("\\[a+b\\]"))
    }

    @Test fun boldItalicCodeLink() {
        val r = parseInline("**gras** et *ital* et `c\$d` et [ici](https://x.fr)")
        assertEquals(MdInline.Bold(listOf(MdInline.Text("gras"))), r[0])
        assertEquals(MdInline.Italic(listOf(MdInline.Text("ital"))), r[2])
        assertEquals(MdInline.Code("c\$d"), r[4]) // pas de formule dans le code
        assertEquals(MdInline.Link(listOf(MdInline.Text("ici")), "https://x.fr"), r[6])
    }

    @Test fun mathInsideBold() {
        assertEquals(listOf(MdInline.Bold(listOf(MdInline.Math("x", false)))), parseInline("**\$x\$**"))
    }

    @Test fun unsafeLinkKeepsOnlyItsText() {
        assertEquals(listOf(MdInline.Text("clic")), parseInline("[clic](javascript:alert)"))
    }

    @Test fun snakeCaseIsNotItalic() {
        assertEquals(listOf(MdInline.Text("read_page_now")), parseInline("read_page_now"))
    }

    @Test fun displayMathBlockSingleAndMultiLine() {
        assertEquals(listOf(MdBlock.DisplayMath("a=b")), parseMarkdown("\$\$a=b\$\$"))
        assertEquals(listOf(MdBlock.Para(listOf(MdInline.Text("Avant"))), MdBlock.DisplayMath("a\n=b"), MdBlock.Para(listOf(MdInline.Text("Après")))), parseMarkdown("Avant\n\$\$\na\n=b\n\$\$\nAprès"))
    }

    @Test fun unclosedDisplayMathIsTextWhileStreaming() {
        val b = parseMarkdown("\$\$\na=b")
        assertTrue(b.single() is MdBlock.Para)
    }

    @Test fun displayMathInsideParagraphIsSplit() {
        val b = parseMarkdown("On a \$\$x\$\$ donc fini")
        assertEquals(3, b.size)
        assertEquals(MdBlock.DisplayMath("x"), b[1])
    }

    @Test fun codeFenceKeepsMathUntouched() {
        assertEquals(listOf(MdBlock.Code("kotlin", "val a = \"\$x\$\"")), parseMarkdown("```kotlin\nval a = \"\$x\$\"\n```"))
    }

    @Test fun mathFenceIsDisplayMath() {
        assertEquals(listOf(MdBlock.DisplayMath("E=mc^2")), parseMarkdown("```math\nE=mc^2\n```"))
    }

    @Test fun headingsAndLists() {
        val b = parseMarkdown("# Titre\n- un\n- deux \$x\$\n  - sous\n1. premier")
        assertEquals(MdBlock.Heading(1, listOf(MdInline.Text("Titre"))), b[0])
        val items = (b[1] as MdBlock.Items).items
        assertEquals(4, items.size)
        assertEquals(1, items[2].depth)
        assertEquals("1.", items[3].marker)
    }
}
