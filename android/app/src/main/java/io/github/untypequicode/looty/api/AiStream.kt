package io.github.untypequicode.looty.api

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * Lecture d'un flux SSE ligne à ligne (le serveur envoie `data: {json}` puis une ligne vide).
 * [push] renvoie la charge utile d'un évènement terminé, ou null tant qu'il est incomplet.
 */
class SseParser {
    private val data = StringBuilder()
    private var has = false

    fun push(line: String): String? {
        if (line.isEmpty()) {
            if (!has) return null
            val out = data.toString()
            data.setLength(0); has = false
            return out
        }
        if (line.startsWith(":")) return null // commentaire / maintien de connexion
        if (line.startsWith("data:")) {
            val v = line.substring(5).let { if (it.startsWith(" ")) it.substring(1) else it }
            if (has) data.append('\n')
            data.append(v); has = true
        }
        return null
    }

    /** Fin du flux sans ligne vide finale : l'évènement en cours est tout de même rendu. */
    fun finish(): String? = if (has) data.toString().also { data.setLength(0); has = false } else null
}

/** Une ligne du diff d'une suggestion (op : ' ', '+' ou '-'). */
@Serializable data class AiDiffLine(val op: String = " ", val text: String = "")
@Serializable data class AiSuggestion(val id: String, val page_id: String = "", val diff: List<AiDiffLine> = emptyList())

/** Autorisation que l'assistant a demandée sans l'avoir (carte « Autoriser » dans la discussion). */
data class AiGrantNeed(val kind: String, val pageId: String?, val workspaceId: String?, val setId: String?, val personal: Boolean, val title: String, val operation: String?)

sealed interface AiEvent {
    data class Text(val delta: String) : AiEvent
    data class Tool(val name: String, val ok: Boolean, val pageId: String?) : AiEvent
    data class Suggestion(val suggestion: AiSuggestion) : AiEvent
    data class PageChanged(val pageId: String) : AiEvent
    data class GrantNeeded(val need: AiGrantNeed) : AiEvent
    data class Title(val title: String) : AiEvent
    data class Error(val message: String) : AiEvent
    data object Done : AiEvent
}

private fun JsonElement?.str(): String? = (this as? JsonPrimitive)?.takeIf { it.isString }?.contentOrNull

fun parseGrantNeed(o: JsonObject): AiGrantNeed? {
    val kind = o["kind"].str() ?: return null
    return AiGrantNeed(kind, o["page_id"].str(), o["workspace_id"].str(), o["set_id"].str(), (o["personal"] as? JsonPrimitive)?.booleanOrNull == true, o["title"].str().orEmpty(), o["operation"].str())
}

/** Charge utile JSON d'un évènement → évènement typé ; null pour un type inconnu ou un JSON invalide. */
fun parseAiEvent(json: Json, data: String): AiEvent? {
    val o = runCatching { json.parseToJsonElement(data).jsonObject }.getOrNull() ?: return null
    return when (o["type"].str()) {
        "text" -> AiEvent.Text(o["delta"].str().orEmpty())
        "tool" -> AiEvent.Tool(o["name"].str().orEmpty(), (o["ok"] as? JsonPrimitive)?.booleanOrNull != false, o["page_id"].str())
        "suggestion" -> runCatching { json.decodeFromJsonElement(AiSuggestion.serializer(), o["suggestion"]!!) }.getOrNull()?.let { AiEvent.Suggestion(it) }
        "page_changed" -> o["page_id"].str()?.let { AiEvent.PageChanged(it) }
        "grant_needed" -> (o["grant"] as? JsonObject)?.let(::parseGrantNeed)?.let { AiEvent.GrantNeeded(it) }
        "title" -> AiEvent.Title(o["title"].str().orEmpty())
        "error" -> AiEvent.Error(o["message"].str().orEmpty())
        "done" -> AiEvent.Done
        else -> null
    }
}
