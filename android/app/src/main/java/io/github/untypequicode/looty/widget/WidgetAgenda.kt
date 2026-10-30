package io.github.untypequicode.looty.widget

import android.content.Context
import io.github.untypequicode.looty.api.Api
import io.github.untypequicode.looty.api.ApiException
import io.github.untypequicode.looty.api.CalEvents
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.time.OffsetDateTime
import java.time.ZoneId
import java.time.ZoneOffset

/** Une ligne de l'agenda : événement (heure) ou échéance de tâche (journée). */
class AgendaItem(val key: String, val title: String, val day: LocalDate, val time: LocalTime?, val end: LocalTime?, val isTask: Boolean, val taskId: String?, val done: Boolean, val route: String)

/** Agenda des prochains jours + compteurs du jour (widgets « Agenda » et « Aujourd'hui »). */
class WidgetAgenda(val workspace: String?, val items: List<AgendaItem>, val signedOut: Boolean = false, val failed: Boolean = false) {
    val today: LocalDate get() = LocalDate.now()
    val openTasksToday: Int get() = items.count { it.isTask && !it.done && it.day <= today }
    val overdue: Int get() = items.count { it.isTask && !it.done && it.day < today }
    val eventsToday: Int get() = items.count { !it.isTask && it.day == today }
    /** Prochain événement horaire à venir aujourd'hui (sinon le premier des jours suivants). */
    fun nextEvent(now: LocalDateTime = LocalDateTime.now()): AgendaItem? =
        items.filter { !it.isTask }.firstOrNull { it.day > now.toLocalDate() || (it.day == now.toLocalDate() && (it.time == null || it.end?.isAfter(now.toLocalTime()) != false)) }

    companion object {
        private const val DAYS = 7L

        suspend fun load(ctx: Context): WidgetAgenda {
            val prefs = ctx.getSharedPreferences("looty", 0)
            val cache = ctx.getSharedPreferences("looty.widget", 0)
            val api = Api(ctx)
            return try {
                withTimeoutOrNull(10_000) {
                    val ws = prefs.getString("workspace", null) ?: api.get<io.github.untypequicode.looty.api.Workspaces>("/api/workspaces").workspaces.firstOrNull()?.id
                        ?: return@withTimeoutOrNull WidgetAgenda(null, emptyList())
                    val zone = ZoneId.systemDefault()
                    val from = LocalDate.now().atStartOfDay(zone).toInstant()
                    val to = from.plusSeconds(DAYS * 86_400)
                    val r = api.get<CalEvents>("/api/workspaces/$ws/calendars/events?from=${enc(from)}&to=${enc(to)}")
                    val items = mutableListOf<AgendaItem>()
                    for (e in r.events) {
                        val s = instant(e.starts_at)?.atZone(zone) ?: continue
                        val en = instant(e.ends_at)?.atZone(zone)
                        items += AgendaItem("e${e.id}${e.starts_at}", e.summary, s.toLocalDate(), if (e.all_day) null else s.toLocalTime(), if (e.all_day) null else en?.toLocalTime(), false, null, false, "/w/$ws/calendar")
                    }
                    for (t in r.tasks) {
                        val d = day(t.due_date) ?: continue
                        items += AgendaItem("t${t.id}", t.title, d, null, null, true, t.id, t.is_completed, "/w/$ws/task/${t.id}")
                    }
                    val sorted = items.sortedWith(compareBy({ it.day }, { it.isTask }, { it.time ?: LocalTime.MIN }))
                    cache.edit().putString("agenda", encode(ws, sorted)).apply()
                    WidgetAgenda(ws, sorted)
                } ?: cached(cache)
            } catch (e: ApiException) {
                if (e.status == 401) WidgetAgenda(null, emptyList(), signedOut = true) else cached(cache)
            } catch (_: Exception) {
                cached(cache)
            }
        }

        private fun enc(i: Instant) = java.net.URLEncoder.encode(i.toString(), "UTF-8")
        private fun instant(iso: String): Instant? = runCatching { OffsetDateTime.parse(iso).toInstant() }.getOrNull()
            ?: runCatching { LocalDateTime.parse(iso).toInstant(ZoneOffset.UTC) }.getOrNull()
        private fun day(iso: String): LocalDate? = runCatching { OffsetDateTime.parse(iso).atZoneSameInstant(ZoneId.systemDefault()).toLocalDate() }.getOrNull()
            ?: runCatching { LocalDate.parse(iso.take(10)) }.getOrNull()

        private fun encode(ws: String, items: List<AgendaItem>) = JSONObject().put("ws", ws).put("items", JSONArray().also { a ->
            items.forEach {
                a.put(JSONObject().put("k", it.key).put("t", it.title).put("d", it.day.toString()).put("s", it.time?.toString()).put("e", it.end?.toString())
                    .put("task", it.isTask).put("id", it.taskId).put("done", it.done).put("r", it.route))
            }
        }).toString()

        private fun cached(cache: android.content.SharedPreferences): WidgetAgenda {
            val raw = cache.getString("agenda", null) ?: return WidgetAgenda(null, emptyList(), failed = true)
            return runCatching {
                val o = JSONObject(raw)
                val a = o.getJSONArray("items")
                WidgetAgenda(o.getString("ws"), List(a.length()) { i ->
                    val x = a.getJSONObject(i)
                    fun str(k: String) = x.optString(k).takeIf { it.isNotBlank() && it != "null" }
                    AgendaItem(x.getString("k"), x.getString("t"), LocalDate.parse(x.getString("d")), str("s")?.let(LocalTime::parse), str("e")?.let(LocalTime::parse),
                        x.getBoolean("task"), str("id"), x.getBoolean("done"), x.getString("r"))
                }, failed = true)
            }.getOrDefault(WidgetAgenda(null, emptyList(), failed = true))
        }
    }
}
