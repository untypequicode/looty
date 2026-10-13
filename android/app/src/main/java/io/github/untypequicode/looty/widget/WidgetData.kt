package io.github.untypequicode.looty.widget

import android.content.Context
import io.github.untypequicode.looty.api.Api
import io.github.untypequicode.looty.api.ApiException
import io.github.untypequicode.looty.api.MyTasks
import io.github.untypequicode.looty.api.Workspaces
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONArray
import org.json.JSONObject
import java.time.LocalDate
import java.time.OffsetDateTime
import java.time.ZoneId

class WidgetTask(val id: String, val title: String, val due: LocalDate?)

/** Ce que le widget affiche. Dernier résultat gardé sur l'appareil pour l'afficher hors ligne. */
class WidgetData(val workspace: String?, val tasks: List<WidgetTask>, val signedOut: Boolean = false, val failed: Boolean = false) {
    companion object {
        private const val MAX = 12

        suspend fun load(ctx: Context): WidgetData {
            val prefs = ctx.getSharedPreferences("looty", 0)
            val cache = ctx.getSharedPreferences("looty.widget", 0)
            val api = Api(ctx)
            return try {
                withTimeoutOrNull(10_000) {
                    val ws = prefs.getString("workspace", null) ?: api.get<Workspaces>("/api/workspaces").workspaces.firstOrNull()?.id
                    if (ws == null) return@withTimeoutOrNull WidgetData(null, emptyList())
                    val tasks = api.get<MyTasks>("/api/workspaces/$ws/my_tasks?scope=mine").tasks
                        .filter { !it.is_completed && !it.done_by_me }
                        .take(MAX)
                        .map { WidgetTask(it.id, it.title, it.due_date?.let(::day)) }
                    cache.edit().putString("data", encode(ws, tasks)).apply()
                    WidgetData(ws, tasks)
                } ?: cached(cache)
            } catch (e: ApiException) {
                if (e.status == 401) WidgetData(null, emptyList(), signedOut = true) else cached(cache)
            } catch (_: Exception) {
                cached(cache)
            }
        }

        private fun day(iso: String): LocalDate? = runCatching { OffsetDateTime.parse(iso).atZoneSameInstant(ZoneId.systemDefault()).toLocalDate() }.getOrNull()
            ?: runCatching { LocalDate.parse(iso.take(10)) }.getOrNull()

        private fun encode(ws: String, tasks: List<WidgetTask>) = JSONObject().put("ws", ws).put("tasks", JSONArray().also { arr ->
            tasks.forEach { arr.put(JSONObject().put("id", it.id).put("title", it.title).put("due", it.due?.toString())) }
        }).toString()

        private fun cached(cache: android.content.SharedPreferences): WidgetData {
            val raw = cache.getString("data", null) ?: return WidgetData(null, emptyList(), failed = true)
            return runCatching {
                val o = JSONObject(raw)
                val arr = o.getJSONArray("tasks")
                WidgetData(o.getString("ws"), List(arr.length()) { i ->
                    val t = arr.getJSONObject(i)
                    WidgetTask(t.getString("id"), t.getString("title"), t.optString("due").takeIf { it.isNotBlank() && it != "null" }?.let { LocalDate.parse(it) })
                })
            }.getOrDefault(WidgetData(null, emptyList(), failed = true))
        }
    }
}
