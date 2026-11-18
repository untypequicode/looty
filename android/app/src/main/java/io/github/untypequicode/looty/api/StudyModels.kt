package io.github.untypequicode.looty.api

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/** Entraînements (GLOSSARY) : types d'entraînement et d'éléments, miroir de backend/src/study.rs. */
object StudyKind {
    const val QCM = "qcm"
    const val QUESTIONNAIRE = "questionnaire"
    const val FLASHCARDS = "flashcards"
    const val SUMMARY = "summary"
    fun isCards(kind: String) = kind == FLASHCARDS || kind == SUMMARY
}

/** Ligne de la liste (GET /api/study/sets) : l'entraînement sans ses éléments. */
@Serializable data class StudySummary(
    val id: String,
    val workspace_id: String,
    val title: String = "",
    val kind: String = StudyKind.QCM,
    val private_to: String? = null,
    val updated_at: String = "",
    val item_count: Int = 0,
    val last_score: Int? = null,
    val last_max: Int? = null,
    val can_edit: Boolean = false,
)

@Serializable data class StudyChoice(val id: String, val text: String)

/** Élément : `answer` est absent (null) pour qui ne peut que s'entraîner, sauf le verso des fiches. */
@Serializable data class StudyItem(
    val id: String,
    val type: String,
    val prompt: String = "",
    val choices: List<StudyChoice>? = null,
    val answer: JsonElement? = null,
    val explanation: String = "",
)

@Serializable data class StudySet(
    val id: String,
    val workspace_id: String,
    val title: String = "",
    val kind: String = StudyKind.QCM,
    val private_to: String? = null,
    val can_edit: Boolean = false,
    val is_owner: Boolean = false,
    val items: List<StudyItem> = emptyList(),
)

/** Tentative (privée à la personne). `pending` : faite hors ligne, pas encore envoyée (jamais envoyée par le serveur). */
@Serializable data class StudyAttempt(
    val id: String,
    val started_at: String = "",
    val finished_at: String? = null,
    val score: Int = 0,
    val max_score: Int = 0,
    val pending: Boolean = false,
)

@Serializable data class StudyCorrection(val item_id: String, val correct: Boolean? = null, val answer: JsonElement? = null, val explanation: String = "")

@Serializable data class AttemptResult(val attempt: StudyAttempt, val correction: List<StudyCorrection> = emptyList())

/** État d'une fiche pour la personne : facilité, intervalle (jours) et échéance. */
@Serializable data class CardState(val item_id: String, val ease: Float = 2.5f, val interval_days: Int = 0, val due_at: String = "")

@Serializable data class StudyLink(val id: String, val target_kind: String, val target_id: String, val title: String = "")
