package io.github.untypequicode.looty.api

import io.github.untypequicode.looty.offline.Pending
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.add
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

class StudyLogicTest {
    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false; coerceInputValues = true }
    private fun obj(build: kotlinx.serialization.json.JsonObjectBuilder.() -> Unit): JsonElement = buildJsonObject(build)
    private fun s(v: String): JsonElement = JsonPrimitive(v)
    private fun arr(vararg v: String): JsonElement = JsonArray(v.map { JsonPrimitive(it) })

    @Test fun singleChoiceIsScored() {
        val answer = obj { put("choice", "b") }
        assertEquals(true, StudyLogic.scoreItem("single", answer, s("b")))
        assertEquals(false, StudyLogic.scoreItem("single", answer, s("a")))
        assertEquals(false, StudyLogic.scoreItem("single", answer, null))
    }

    @Test fun multiNeedsExactlyTheRightChoices() {
        val answer = obj { putJsonArray("choices") { add("a"); add("c") } }
        assertEquals(true, StudyLogic.scoreItem("multi", answer, arr("c", "a")))
        assertEquals(false, StudyLogic.scoreItem("multi", answer, arr("a")))
        assertEquals(false, StudyLogic.scoreItem("multi", answer, arr("a", "b", "c")))
        assertEquals(false, StudyLogic.scoreItem("multi", answer, arr("a", "a", "c")))
        assertEquals(false, StudyLogic.scoreItem("multi", answer, null))
    }

    @Test fun textIgnoresCaseAccentsAndPunctuation() {
        val answer = obj { putJsonArray("accepted") { add("Paris"); add("la ville de Paris") } }
        assertEquals(true, StudyLogic.scoreItem("text", answer, s("  PARIS! ")))
        assertEquals(true, StudyLogic.scoreItem("text", answer, s("La  ville de   paris")))
        assertEquals(false, StudyLogic.scoreItem("text", answer, s("Lyon")))
        assertEquals(false, StudyLogic.scoreItem("text", answer, s("")))
        assertEquals("ete cafe", StudyLogic.foldAnswer("  Été, café ! "))
    }

    @Test fun openQuestionAndFlashcardAreNotScored() {
        assertNull(StudyLogic.scoreItem("text", obj { putJsonArray("accepted") {} }, s("x")))
        assertNull(StudyLogic.scoreItem("flashcard", obj { put("back", "b") }, null))
        assertNull(StudyLogic.scoreItem("single", null, s("a")))
        assertNull(StudyLogic.scoreItem("single", JsonNull, s("a")))
    }

    private fun item(id: String, type: String, answer: JsonElement?) = StudyItem(id, type, "q", listOf(StudyChoice("a", "A"), StudyChoice("b", "B")), answer)

    @Test fun localCorrectionNeedsTheExpectedAnswers() {
        val withAnswers = StudySet("s", "w", items = listOf(item("1", "single", obj { put("choice", "a") }), item("2", "single", obj { put("choice", "b") })))
        val r = StudyLogic.correctLocally(withAnswers, mapOf("1" to s("a"), "2" to s("a")))!!
        assertEquals(1 to 2, r.first)
        assertEquals(listOf(true, false), r.second.map { it.correct })
        // Qui ne peut que s'entraîner : le serveur retire les réponses, rien à corriger ici.
        val stripped = StudySet("s", "w", items = listOf(item("1", "single", null)))
        assertFalse(StudyLogic.canCorrectLocally(stripped))
        assertNull(StudyLogic.correctLocally(stripped, mapOf("1" to s("a"))))
    }

    @Test fun spacedRepetitionMatchesTheServer() {
        // Miroir du test `spaced_repetition_moves_the_due_date` et de `next_review` (backend/src/study.rs).
        val again = StudyLogic.nextReview(2.5f, 10, "again")!!
        assertEquals(2.3f, again.first, 1e-6f); assertEquals(0, again.second); assertEquals(600L, again.third)
        val hard = StudyLogic.nextReview(2.5f, 10, "hard")!!
        assertEquals(2.35f, hard.first, 1e-6f); assertEquals(12, hard.second); assertEquals(12 * 86_400L, hard.third)
        val good = StudyLogic.nextReview(2.5f, 0, "good")!!
        assertEquals(2.5f, good.first, 1e-6f); assertEquals(1, good.second)
        assertEquals(25, StudyLogic.nextReview(2.5f, 10, "good")!!.second)
        val easy = StudyLogic.nextReview(2.5f, 0, "easy")!!
        assertEquals(2.65f, easy.first, 1e-6f); assertEquals(4, easy.second)
        assertEquals(3.5f, StudyLogic.nextReview(3.45f, 1, "easy")!!.first, 1e-6f)
        assertEquals(1.3f, StudyLogic.nextReview(1.4f, 1, "again")!!.first, 1e-6f)
        assertNull(StudyLogic.nextReview(2.5f, 1, "nope"))
    }

    @Test fun reviewUpdatesTheLocalCardAndDueDate() {
        val now = Instant.parse("2026-11-18T10:00:00Z")
        val first = StudyLogic.review(null, "i1", "good", now)!!
        assertEquals(CardState("i1", 2.5f, 1, "2026-11-19T10:00:00Z"), first)
        assertTrue(StudyLogic.isDue(first, now.plusSeconds(86_400)))
        assertFalse(StudyLogic.isDue(first, now.plusSeconds(3_600)))
        assertTrue(StudyLogic.isDue(null, now))
        assertEquals("2026-11-18T10:10:00Z", StudyLogic.review(first, "i1", "again", now)!!.due_at)
        assertNull(StudyLogic.review(null, "i1", "bad", now))
    }

    @Test fun expectedAnswerIsAReadableText() {
        val single = item("1", "single", null)
        assertEquals("B", StudyLogic.expectedText(single, obj { put("choice", "b") }))
        assertEquals("A, B", StudyLogic.expectedText(item("2", "multi", null), obj { putJsonArray("choices") { add("a"); add("b") } }))
        assertEquals("x / y", StudyLogic.expectedText(item("3", "text", null), obj { putJsonArray("accepted") { add("x"); add("y") } }))
    }

    @Test fun statsAverageTheScoredAttempts() {
        val st = StudyLogic.stats(listOf(StudyAttempt("1", score = 1, max_score = 2), StudyAttempt("2", score = 2, max_score = 2), StudyAttempt("3", pending = true)))
        assertEquals(3, st.count); assertEquals(75, st.average); assertEquals(100, st.best)
    }

    @Test fun modelsReadTheServerPayload() {
        val set = json.decodeFromString<StudySet>("""{"id":"s","workspace_id":"w","title":"T","kind":"qcm","private_to":null,"can_edit":false,"is_owner":true,"items":[{"id":"i","position":0,"type":"single","prompt":"p","choices":[{"id":"a","text":"A"}],"answer":null,"explanation":""}]}""")
        assertNull(set.items[0].answer)
        assertEquals("A", set.items[0].choices!![0].text)
        val res = json.decodeFromString<AttemptResult>("""{"attempt":{"id":"a","set_id":"s","started_at":"x","finished_at":null,"score":1,"max_score":2,"answers":{}},"correction":[{"item_id":"i","correct":null,"answer":{"choice":"a"},"explanation":"e"}]}""")
        assertEquals(1, res.attempt.score); assertNull(res.correction[0].correct); assertFalse(res.attempt.pending)
    }

    @Test fun queuedWritesKeepTheirExactBodyThroughTheOutboxFile() {
        val body = StudyLogic.attemptBody(mapOf("i1" to s("a"), "i2" to arr("a", "c"), "i3" to s("Paris")), "2026-11-18T10:00:00Z")
        val review = Pending("p1", "POST", "/api/study/items/i1/review", StudyLogic.reviewBody("good").toString(), "review")
        val attempt = Pending("p2", "POST", "/api/study/sets/s/attempts", body.toString(), "Réseaux")
        val back = json.decodeFromString<List<Pending>>(json.encodeToString(listOf(review, attempt)))
        assertEquals(listOf(review, attempt), back)
        assertEquals("good", json.parseToJsonElement(back[0].body!!).jsonObject["rating"]!!.jsonPrimitive.content)
        val sent = json.parseToJsonElement(back[1].body!!).jsonObject
        assertEquals("2026-11-18T10:00:00Z", sent["started_at"]!!.jsonPrimitive.content)
        assertEquals(arr("a", "c"), sent["answers"]!!.jsonObject["i2"])
    }
}
