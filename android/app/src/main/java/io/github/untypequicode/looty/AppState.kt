package io.github.untypequicode.looty

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.untypequicode.looty.api.*
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.add
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

/** État d'un chargement : données (éventuellement anciennes) + chargement + erreur. */
data class Loadable<T>(val data: T? = null, val loading: Boolean = false, val error: String? = null)

sealed interface Session {
    data object Loading : Session
    data object NeedServer : Session
    data object NeedLogin : Session
    data class Offline(val message: String) : Session
    data class Ready(val me: User, val workspaces: List<Workspace>, val workspaceId: String) : Session
}

/**
 * État partagé des écrans natifs : session, espace courant, données des écrans.
 * Les écritures passent par l'API, puis les listes concernées sont rechargées.
 */
class AppState(app: Application) : AndroidViewModel(app) {
    val api = Api(app)
    private val prefs = app.getSharedPreferences("looty", 0)

    var session by mutableStateOf<Session>(Session.Loading); private set
    var members by mutableStateOf<Map<String, Member>>(emptyMap()); private set
    var overview by mutableStateOf(Loadable<Overview>()); private set
    var tasks by mutableStateOf(Loadable<List<MyTask>>()); private set
    var taskScope by mutableStateOf(prefs.getString("taskScope", "mine") ?: "mine"); private set
    var pages by mutableStateOf(Loadable<List<Page>>()); private set
    var inbox by mutableStateOf(Loadable<List<Notification>>()); private set
    var upcoming by mutableStateOf(Loadable<List<CalEvent>>()); private set
    var boards by mutableStateOf<List<BoardSummary>>(emptyList()); private set

    val ws: String? get() = (session as? Session.Ready)?.workspaceId
    val me: User? get() = (session as? Session.Ready)?.me
    val unread: Int get() = inbox.data?.count { it.read_at == null } ?: 0

    fun start() {
        if (Config.server(getApplication()) == null) { session = Session.NeedServer; return }
        viewModelScope.launch {
            session = Session.Loading
            session = try {
                val me = api.get<Me>("/api/auth/me")
                val user = me.user
                if (!me.authenticated || user == null) Session.NeedLogin
                else {
                    val list = api.get<Workspaces>("/api/workspaces").workspaces
                    val saved = prefs.getString("workspace", null)
                    val wsId = list.firstOrNull { it.id == saved }?.id ?: list.firstOrNull()?.id
                    if (wsId == null) Session.Offline("no_workspace") else Session.Ready(user, list, wsId)
                }
            } catch (e: ApiException) {
                if (e.status == 401) Session.NeedLogin else Session.Offline(e.code)
            } catch (e: Exception) {
                Session.Offline(e.message ?: "offline")
            }
            if (session is Session.Ready) refreshAll()
        }
    }

    fun switchWorkspace(id: String) {
        val s = session as? Session.Ready ?: return
        prefs.edit().putString("workspace", id).apply()
        session = s.copy(workspaceId = id)
        overview = Loadable(); tasks = Loadable(); pages = Loadable(); inbox = Loadable(); upcoming = Loadable()
        refreshAll()
    }

    fun refreshAll() {
        loadMembers(); loadOverview(); loadTasks(); loadPages(); loadInbox(); loadUpcoming(); loadBoards()
    }

    private fun <T> load(get: () -> Loadable<T>, set: (Loadable<T>) -> Unit, block: suspend () -> T) {
        set(get().copy(loading = true, error = null))
        viewModelScope.launch {
            set(try { Loadable(block()) } catch (e: ApiException) { get().copy(loading = false, error = e.code) } catch (e: Exception) { get().copy(loading = false, error = "offline") })
        }
    }

    fun loadMembers() {
        val w = ws ?: return
        viewModelScope.launch { runCatching { members = api.get<Members>("/api/workspaces/$w/members").members.associateBy { it.user_id } } }
    }
    fun loadOverview() { val w = ws ?: return; load({ overview }, { overview = it }) { api.get("/api/workspaces/$w/overview") } }
    fun loadTasks() { val w = ws ?: return; load({ tasks }, { tasks = it }) { api.get<MyTasks>("/api/workspaces/$w/my_tasks?scope=$taskScope").tasks } }
    fun loadPages() { val w = ws ?: return; load({ pages }, { pages = it }) { api.get<List<Page>>("/api/workspaces/$w/pages") } }
    fun loadInbox() { val w = ws ?: return; load({ inbox }, { inbox = it }) { api.get<List<Notification>>("/api/notifications?workspace_id=$w") } }
    fun loadBoards() { val w = ws ?: return; viewModelScope.launch { runCatching { boards = api.get<List<BoardSummary>>("/api/workspaces/$w/boards") } } }
    /** Événements d'aujourd'hui et de demain (calendriers de l'espace et personnels). */
    fun loadUpcoming() {
        val w = ws ?: return
        val zone = ZoneId.systemDefault()
        val from = LocalDate.now().atStartOfDay(zone).toInstant()
        val to = from.plusSeconds(2 * 86400)
        load({ upcoming }, { upcoming = it }) {
            api.get<CalEvents>("/api/workspaces/$w/calendars/events?from=$from&to=$to").events
                .filter { Instant.parse(it.ends_at) >= Instant.now() || it.all_day }
                .sortedBy { it.starts_at }
        }
    }

    fun setScope(scope: String) {
        taskScope = scope
        prefs.edit().putString("taskScope", scope).apply()
        loadTasks()
    }

    /** Coche / décoche (pour une tâche individuelle : ma part seulement). */
    fun toggleTask(id: String, onDone: () -> Unit = {}) {
        viewModelScope.launch {
            runCatching { api.send("PUT", "/api/boards/tasks/$id/toggle_validation") }
            // Laisse voir la coche avant que la tâche quitte la liste.
            kotlinx.coroutines.delay(900)
            loadTasks(); loadOverview(); onDone()
        }
    }

    fun markRead(n: Notification) {
        if (n.read_at != null) return
        inbox = inbox.copy(data = inbox.data?.map { if (it.id == n.id) it.copy(read_at = Instant.now().toString()) else it })
        viewModelScope.launch { runCatching { api.send("PUT", "/api/notifications/${n.id}/read", buildJsonObject { put("read", true) }) } }
    }

    fun markAllRead() {
        val w = ws ?: return
        inbox = inbox.copy(data = inbox.data?.map { it.copy(read_at = it.read_at ?: Instant.now().toString()) })
        viewModelScope.launch { runCatching { api.send("POST", "/api/notifications/read-all?workspace_id=$w") }; loadInbox() }
    }

    /** Ajout rapide : première colonne du tableau choisi. */
    suspend fun createTask(boardId: String, title: String, due: LocalDate?, assignMe: Boolean): Result<String> = runCatching {
        val board = api.get<Board>("/api/boards/$boardId")
        val col = board.columns.minByOrNull { it.position } ?: throw ApiException(0, "no_column")
        val body = buildJsonObject {
            put("title", title.trim())
            put("at_top", true)
            if (due != null) put("due_date", due.atTime(12, 0).atZone(ZoneId.systemDefault()).toInstant().toString())
            if (assignMe) me?.let { putJsonArray("assignees") { add(it.id) } }
        }
        prefs.edit().putString("lastBoard", boardId).apply()
        val created = api.post<Created>("/api/boards/columns/${col.id}/tasks", body)
        loadTasks(); loadOverview()
        created.id
    }
    val lastBoard: String? get() = prefs.getString("lastBoard", null)

    suspend fun createPage(): Result<String> = runCatching {
        val w = ws ?: throw ApiException(0, "no_workspace")
        val p = api.post<Page>("/api/workspaces/$w/pages", buildJsonObject { put("title", "") })
        loadPages()
        p.id
    }

    fun logout(onDone: () -> Unit) {
        viewModelScope.launch {
            runCatching { api.send("POST", "/api/auth/logout") }
            android.webkit.CookieManager.getInstance().removeAllCookies(null)
            session = Session.NeedLogin
            onDone()
        }
    }
}
