package io.github.untypequicode.looty.api

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

/** Connexion IA (réglée sur le web ; l'application ne montre jamais la clé). */
@Serializable data class AiConnection(val id: String, val provider: String = "", val label: String = "", val model: String = "", val has_key: Boolean = false)

@Serializable data class AiConversation(
    val id: String,
    val connection_id: String? = null,
    val context_kind: String = "none",
    val context_id: String? = null,
    val title: String = "",
    val updated_at: String? = null,
)

@Serializable data class AiMessage(val id: String, val role: String, val content: String = "", val data: JsonElement? = null)

/** Autorisation d'écriture sur une page (mode : suggest | append | replace). */
@Serializable data class AiGrant(val id: String, val page_id: String, val page_title: String = "", val workspace_id: String = "", val mode: String = "suggest", val expires_at: String = "")

/** Autorisation pour les entraînements (kind : create | update). */
@Serializable data class AiStudyGrant(
    val id: String,
    val kind: String,
    val workspace_id: String? = null,
    val workspace_name: String? = null,
    val personal: Boolean = false,
    val set_id: String? = null,
    val set_title: String? = null,
    val expires_at: String = "",
)

@Serializable data class AiConversationDetail(
    val conversation: AiConversation,
    val messages: List<AiMessage> = emptyList(),
    val grants: List<AiGrant> = emptyList(),
    val study_grants: List<AiStudyGrant> = emptyList(),
    val suggestions: List<AiSuggestion> = emptyList(),
)

/** Entraînement proposé pour une autorisation de modification. */
@Serializable data class AiStudySet(val id: String, val title: String = "", val can_edit: Boolean? = null)
