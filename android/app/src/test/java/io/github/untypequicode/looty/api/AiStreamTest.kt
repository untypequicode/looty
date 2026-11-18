package io.github.untypequicode.looty.api

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AiStreamTest {
    private val json = Json { ignoreUnknownKeys = true }

    private fun events(raw: String): List<String> {
        val p = SseParser()
        val out = mutableListOf<String>()
        raw.split("\n").forEach { p.push(it)?.let(out::add) }
        p.finish()?.let(out::add)
        return out
    }

    @Test fun framesAreSplitOnBlankLines() {
        assertEquals(listOf("{\"a\":1}", "{\"b\":2}"), events("data: {\"a\":1}\n\ndata: {\"b\":2}\n\n"))
    }

    @Test fun keepAliveCommentsAndOtherFieldsAreIgnored() {
        assertEquals(listOf("x"), events(": keep-alive\n\nevent: ping\nid: 3\ndata: x\n\n"))
    }

    @Test fun multiLineDataIsJoined() {
        assertEquals(listOf("a\nb"), events("data: a\ndata: b\n\n"))
    }

    @Test fun lastFrameWithoutBlankLineIsKeptAtEnd() {
        assertEquals(listOf("{\"type\":\"done\"}"), events("data:{\"type\":\"done\"}"))
    }

    @Test fun textEvent() {
        assertEquals(AiEvent.Text("Bonjour \$x\$"), parseAiEvent(json, "{\"type\":\"text\",\"delta\":\"Bonjour \$x\$\"}"))
    }

    @Test fun toolEventWithNullPage() {
        assertEquals(AiEvent.Tool("read_page", false, null), parseAiEvent(json, "{\"type\":\"tool\",\"name\":\"read_page\",\"ok\":false,\"page_id\":null,\"study_set_id\":null}"))
    }

    @Test fun suggestionEvent() {
        val e = parseAiEvent(json, "{\"type\":\"suggestion\",\"suggestion\":{\"id\":\"s1\",\"page_id\":\"p1\",\"diff\":[{\"op\":\"-\",\"text\":\"a\"},{\"op\":\"+\",\"text\":\"b\"}]}}") as AiEvent.Suggestion
        assertEquals("s1", e.suggestion.id)
        assertEquals(listOf(AiDiffLine("-", "a"), AiDiffLine("+", "b")), e.suggestion.diff)
    }

    @Test fun grantNeededEvent() {
        val e = parseAiEvent(json, "{\"type\":\"grant_needed\",\"grant\":{\"kind\":\"page\",\"page_id\":\"p\",\"workspace_id\":\"w\",\"title\":\"T\",\"operation\":\"append\"}}") as AiEvent.GrantNeeded
        assertEquals("page", e.need.kind); assertEquals("append", e.need.operation); assertEquals("p", e.need.pageId)
    }

    @Test fun errorDoneAndPageChanged() {
        assertEquals(AiEvent.Error("provider_http_429"), parseAiEvent(json, "{\"type\":\"error\",\"message\":\"provider_http_429\"}"))
        assertEquals(AiEvent.Done, parseAiEvent(json, "{\"type\":\"done\"}"))
        assertEquals(AiEvent.PageChanged("p"), parseAiEvent(json, "{\"type\":\"page_changed\",\"page_id\":\"p\"}"))
    }

    @Test fun unknownOrBrokenIsNull() {
        assertNull(parseAiEvent(json, "{\"type\":\"future\"}"))
        assertNull(parseAiEvent(json, "not json"))
        assertTrue(parseAiEvent(json, "{\"type\":\"text\"}") is AiEvent.Text)
    }
}
