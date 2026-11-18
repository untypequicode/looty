package io.github.untypequicode.looty.api

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.put
import java.time.Instant
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * Logique d'entraînement exécutée sur l'appareil. Le serveur reste la référence quand il répond ;
 * ces fonctions (miroirs de `score_item`, `fold_answer` et `next_review` de backend/src/study.rs)
 * ne servent qu'hors ligne, pour corriger ce qui peut l'être et mettre à jour la copie des fiches.
 */
object StudyLogic {
    const val DEFAULT_EASE = 2.5f
    val RATINGS = listOf("again", "hard", "good", "easy")

    /** Texte comparé sans casse, sans accents, sans ponctuation ni espaces multiples. */
    fun foldAnswer(s: String): String {
        val sb = StringBuilder()
        for (ch in s.trim().lowercase()) {
            sb.append(
                when (ch) {
                    'à', 'â', 'ä', 'á', 'ã' -> 'a'
                    'é', 'è', 'ê', 'ë' -> 'e'
                    'î', 'ï', 'í' -> 'i'
                    'ô', 'ö', 'ó', 'õ' -> 'o'
                    'ù', 'û', 'ü', 'ú' -> 'u'
                    'ç' -> 'c'
                    'ÿ' -> 'y'
                    'ñ' -> 'n'
                    else -> if (ch.isLetterOrDigit()) ch else ' '
                },
            )
        }
        return sb.toString().split(Regex("\\s+")).filter { it.isNotEmpty() }.joinToString(" ")
    }

    private fun strings(e: JsonElement?): List<String>? = (e as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.contentOrNull }

    /** Corrige un élément : null = non noté (fiche, question libre sans réponse attendue, ou réponse inconnue). */
    fun scoreItem(type: String, answer: JsonElement?, given: JsonElement?): Boolean? {
        val a = answer as? JsonObject ?: return null
        return when (type) {
            "single" -> {
                val want = (a["choice"] as? JsonPrimitive)?.contentOrNull ?: return null
                (given as? JsonPrimitive)?.takeIf { it.isString }?.content == want
            }
            "multi" -> {
                val want = strings(a["choices"]) ?: return null
                val got = strings(given) ?: emptyList()
                got.size == got.toSet().size && got.toSet() == want.toSet()
            }
            "text" -> {
                val accepted = strings(a["accepted"]) ?: return null
                if (accepted.isEmpty()) return null
                val g = foldAnswer((given as? JsonPrimitive)?.contentOrNull.orEmpty())
                g.isNotEmpty() && accepted.any { foldAnswer(it) == g }
            }
            else -> null
        }
    }

    /** Les réponses attendues sont connues : pas le cas pour qui ne peut que s'entraîner (le serveur les garde). */
    fun canCorrectLocally(set: StudySet): Boolean =
        set.items.filter { it.type != "flashcard" }.all { it.answer != null && it.answer !is JsonNull }

    /** Correction faite sur l'appareil ; null si les réponses attendues ne sont pas là ou rien n'est noté. */
    fun correctLocally(set: StudySet, answers: Map<String, JsonElement>): Pair<Pair<Int, Int>, List<StudyCorrection>>? {
        if (!canCorrectLocally(set)) return null
        var score = 0
        var maxScore = 0
        val corrections = set.items.map { it ->
            val ok = scoreItem(it.type, it.answer, answers[it.id])
            if (ok != null) { maxScore++; if (ok) score++ }
            StudyCorrection(it.id, ok, it.answer, it.explanation)
        }
        if (maxScore == 0) return null
        return (score to maxScore) to corrections
    }

    /** Nouvel état après une évaluation : (facilité, intervalle en jours, délai en secondes) ; null si note inconnue. */
    fun nextReview(ease: Float, intervalDays: Int, rating: String): Triple<Float, Int, Long>? {
        val (e, i) = when (rating) {
            "again" -> max(ease - 0.2f, 1.3f) to 0
            "hard" -> max(ease - 0.15f, 1.3f) to max((intervalDays * 1.2f).roundToInt(), 1)
            "good" -> ease to max((intervalDays * ease).roundToInt(), 1)
            "easy" -> min(ease + 0.15f, 3.5f) to max((intervalDays * ease * 1.3f).roundToInt(), 4)
            else -> return null
        }
        return Triple(e, i, if (i == 0) 600L else i * 86_400L)
    }

    /** État de la fiche après évaluation, calculé comme le serveur (copie locale ; le serveur fait foi au prochain chargement). */
    fun review(current: CardState?, itemId: String, rating: String, now: Instant = Instant.now()): CardState? {
        val (e, i, delay) = nextReview(current?.ease ?: DEFAULT_EASE, current?.interval_days ?: 0, rating) ?: return null
        return CardState(itemId, e, i, now.plusSeconds(delay).toString())
    }

    /** Fiche à revoir : jamais vue, ou échéance passée. */
    fun isDue(state: CardState?, now: Instant = Instant.now()): Boolean {
        val due = state?.due_at?.takeIf { it.isNotEmpty() }?.let { runCatching { Instant.parse(it) }.getOrNull() } ?: return true
        return !due.isAfter(now)
    }

    fun attemptBody(answers: Map<String, JsonElement>, startedAt: String): JsonElement = buildJsonObject {
        put("answers", JsonObject(answers))
        put("started_at", startedAt)
    }

    fun reviewBody(rating: String): JsonElement = buildJsonObject { put("rating", rating) }

    /** Réponse attendue en un seul texte (une formule `$…$` ne doit pas être coupée entre deux fragments). */
    fun expectedText(item: StudyItem, answer: JsonElement?): String {
        val a = answer as? JsonObject ?: return ""
        return when (item.type) {
            "text" -> strings(a["accepted"]).orEmpty().joinToString(" / ")
            "single" -> item.choices.orEmpty().filter { it.id == (a["choice"] as? JsonPrimitive)?.contentOrNull }.joinToString(", ") { it.text }
            "multi" -> { val ids = strings(a["choices"]).orEmpty(); item.choices.orEmpty().filter { it.id in ids }.joinToString(", ") { it.text } }
            else -> ""
        }
    }

    /** Recto/verso d'une fiche : le verso est `answer.back`. */
    fun back(item: StudyItem): String = ((item.answer as? JsonObject)?.get("back") as? JsonPrimitive)?.contentOrNull.orEmpty()

    data class Stats(val count: Int, val average: Int, val best: Int)

    /** Moyenne et meilleur score (en %) des tentatives corrigées. */
    fun stats(attempts: List<StudyAttempt>): Stats {
        val done = attempts.filter { it.max_score > 0 }
        val pct = done.map { it.score.toDouble() / it.max_score }
        return Stats(attempts.size, if (pct.isEmpty()) 0 else (100 * pct.average()).roundToInt(), if (pct.isEmpty()) 0 else (100 * pct.max()).roundToInt())
    }
}
