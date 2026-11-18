package io.github.untypequicode.looty.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MathHtmlTest {
    @Test fun jsStringEscapesScriptBreakers() {
        val s = MathHtml.jsString("a</script>\"\\\n&")
        assertEquals("\"a\\u003c/script\\u003e\\\"\\\\\\n\\u0026\"", s)
    }

    @Test fun pageEmbedsFormulaSafely() {
        val html = MathHtml.page("\\frac{1}{2}</script><b>", display = true, color = "red")
        assertTrue(html.contains("displayMode:true"))
        assertTrue(html.contains("\\\\frac{1}{2}\\u003c/script"))
        assertFalse(html.contains("</script><b>"))
    }
}
