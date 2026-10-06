package io.github.untypequicode.looty

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.untypequicode.looty.api.*
import kotlinx.coroutines.async
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.add
import kotlinx.serialization.json.JsonObject
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
        loadMembers(); loadOverview(); loadTasks(); loadPages(); loadInbox(); loadUpcoming(); loadBoards(); loadMailAccounts()
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

    // -----------------------------------------------------------------------
    // Tableaux et tâches (écrans natifs G2). Changement appliqué tout de suite à
    // l'écran (copie locale), envoyé au serveur, puis le tableau est rechargé.
    // -----------------------------------------------------------------------

    val boardData = mutableStateMapOf<String, Loadable<BoardFull>>()
    /** Tâche → tableau (pour ouvrir une tâche depuis n'importe où). */
    val taskBoard = mutableStateMapOf<String, String>()
    val comments = mutableStateMapOf<String, List<Comment>>()
    val attachments = mutableStateMapOf<String, List<Attachment>>()
    var lastError by mutableStateOf<String?>(null)

    fun loadBoard(id: String) {
        val cur = boardData[id] ?: Loadable()
        boardData[id] = cur.copy(loading = true, error = null)
        viewModelScope.launch {
            boardData[id] = try {
                val b = api.get<BoardFull>("/api/boards/$id")
                b.allTasks.forEach { taskBoard[it.id] = id }
                Loadable(b)
            } catch (e: ApiException) {
                (boardData[id] ?: Loadable()).copy(loading = false, error = e.code)
            } catch (e: Exception) {
                (boardData[id] ?: Loadable()).copy(loading = false, error = "offline")
            }
        }
    }

    /** Ouvre une tâche connue seulement par son identifiant : trouve son tableau. */
    fun ensureTask(taskId: String) {
        val b = taskBoard[taskId]
        if (b != null) { if (boardData[b]?.data == null) loadBoard(b); return }
        viewModelScope.launch {
            runCatching { api.get<TaskEnvelope>("/api/boards/tasks/$taskId") }
                .onSuccess { env -> taskBoard[taskId] = env.board_id; loadBoard(env.board_id) }
                .onFailure { lastError = (it as? ApiException)?.code ?: "offline" }
        }
    }

    fun boardOf(taskId: String): BoardFull? = taskBoard[taskId]?.let { boardData[it]?.data }

    private fun patchLocal(boardId: String, f: (BoardFull) -> BoardFull) {
        val entry = boardData[boardId] ?: return
        val cur = entry.data ?: return
        boardData[boardId] = entry.copy(data = f(cur))
    }

    private fun mapTask(b: BoardFull, id: String, f: (Task) -> Task) =
        b.copy(columns = b.columns.map { c -> c.copy(tasks = c.tasks.map { if (it.id == id) f(it) else it }) })

    /** Écriture : copie locale, appel, rechargement (qui corrige en cas de refus). */
    private fun mutate(boardId: String, local: (BoardFull) -> BoardFull, call: suspend () -> Unit) {
        patchLocal(boardId, local)
        viewModelScope.launch {
            try { call() } catch (e: ApiException) { lastError = e.code } catch (e: Exception) { lastError = "offline" }
            loadBoard(boardId)
            loadTasks(); loadOverview()
        }
    }

    fun updateTask(task: Task, body: JsonObject, local: (Task) -> Task) {
        val b = taskBoard[task.id] ?: return
        mutate(b, { mapTask(it, task.id, local) }) { api.send("PUT", "/api/boards/tasks/${task.id}", body) }
    }

    fun toggleDone(task: Task) {
        val b = taskBoard[task.id] ?: return
        val meId = me?.id ?: return
        mutate(b, { bd -> mapTask(bd, task.id) { t ->
            if (t.is_per_person) t.copy(validations = if (meId in t.validations) t.validations - meId else t.validations + meId)
            else t.copy(is_completed = !t.is_completed)
        } }) { api.send("PUT", "/api/boards/tasks/${task.id}/toggle_validation") }
    }

    fun moveTask(task: Task, columnId: String) {
        val b = taskBoard[task.id] ?: return
        val board = boardData[b]?.data ?: return
        val target = board.column(columnId) ?: return
        // En haut de la colonne d'arrivée.
        val pos = (target.tasks.filter { it.parent_id == null }.minOfOrNull { it.position } ?: 65536.0) / 2
        mutate(b, { bd ->
            val moved = task.copy(column_id = columnId, position = pos)
            bd.copy(columns = bd.columns.map { c ->
                if (c.id == columnId) c.copy(tasks = listOf(moved) + c.tasks.filter { it.id != task.id })
                else c.copy(tasks = c.tasks.filter { it.id != task.id })
            })
        }) { api.send("PUT", "/api/boards/tasks/${task.id}/move", buildJsonObject { put("column_id", columnId); put("position", pos) }) }
    }

    fun setLabel(task: Task, label: Label, on: Boolean) {
        val b = taskBoard[task.id] ?: return
        mutate(b, { bd -> mapTask(bd, task.id) { t -> t.copy(labels = if (on) t.labels + label else t.labels.filter { it.id != label.id }) } }) {
            api.send(if (on) "POST" else "DELETE", "/api/boards/tasks/${task.id}/labels/${label.id}")
        }
    }

    fun setAssignee(task: Task, userId: String, on: Boolean) {
        val b = taskBoard[task.id] ?: return
        mutate(b, { bd -> mapTask(bd, task.id) { t -> t.copy(assignees = if (on) t.assignees + userId else t.assignees - userId) } }) {
            api.send(if (on) "POST" else "DELETE", "/api/boards/tasks/${task.id}/assignees/$userId")
        }
    }

    fun addSubtask(parent: Task, title: String) {
        val b = taskBoard[parent.id] ?: return
        viewModelScope.launch {
            runCatching { api.send("POST", "/api/boards/tasks/${parent.id}/subtasks", buildJsonObject { put("title", title.trim()) }) }
                .onFailure { lastError = (it as? ApiException)?.code ?: "offline" }
            loadBoard(b)
        }
    }

    suspend fun createInColumn(boardId: String, columnId: String, title: String): Result<String> = runCatching {
        val created = api.post<Created>("/api/boards/columns/$columnId/tasks", buildJsonObject { put("title", title.trim()); put("at_top", true) })
        taskBoard[created.id] = boardId
        loadBoard(boardId)
        created.id
    }

    fun deleteTask(task: Task) {
        val b = taskBoard[task.id] ?: return
        mutate(b, { bd -> bd.copy(columns = bd.columns.map { c -> c.copy(tasks = c.tasks.filter { it.id != task.id && it.parent_id != task.id }) }) }) {
            api.send("DELETE", "/api/boards/tasks/${task.id}")
        }
    }

    /** Annulation d'une suppression (corbeille). */
    fun restoreTask(taskId: String) {
        val b = taskBoard[taskId] ?: return
        viewModelScope.launch { runCatching { api.send("POST", "/api/trash/tasks/$taskId/restore") }; loadBoard(b); loadTasks() }
    }

    fun loadComments(taskId: String) {
        viewModelScope.launch {
            runCatching { api.get<List<Comment>>("/api/boards/tasks/$taskId/comments") }.onSuccess { comments[taskId] = it }
            runCatching { api.get<List<Attachment>>("/api/boards/tasks/$taskId/attachments") }.onSuccess { attachments[taskId] = it }
        }
    }

    suspend fun addComment(taskId: String, text: String): Boolean = runCatching {
        api.send("POST", "/api/boards/tasks/$taskId/comments", buildJsonObject { put("content", io.github.untypequicode.looty.ui.Html.fromPlain(text)) })
        loadComments(taskId)
    }.isSuccess

    // -----------------------------------------------------------------------
    // Calendrier (G3)
    // -----------------------------------------------------------------------

    var calSources by mutableStateOf<List<CalendarSource>>(emptyList()); private set
    var calRange by mutableStateOf<Pair<LocalDate, LocalDate>?>(null); private set
    var calEvents by mutableStateOf(Loadable<CalEvents>()); private set
    /** Calendriers masqués (propres à l'appareil) ; « workspace » = calendrier de l'espace. */
    var hiddenCalendars by mutableStateOf(prefs.getStringSet("hiddenCalendars", emptySet()).orEmpty()); private set
    val canWrite: Boolean get() = (session as? Session.Ready)?.let { s -> s.workspaces.firstOrNull { it.id == s.workspaceId }?.role } in setOf("owner", "admin", "member")

    fun loadCalendars() {
        val w = ws ?: return
        viewModelScope.launch {
            runCatching {
                // Calendriers Looty de l'espace (par défaut, thématiques, tableaux) puis abonnements.
                val looty = runCatching { api.get<List<LootyCalendar>>("/api/workspaces/$w/planning/calendars") }.getOrDefault(emptyList())
                    .map { CalendarSource(it.id, if (it.is_default) "" else it.name, it.color, "looty", "workspace") }
                calSources = looty + api.get<List<CalendarSource>>("/api/workspaces/$w/calendars")
            }
        }
    }

    fun loadEvents(from: LocalDate, to: LocalDate) {
        val w = ws ?: return
        calRange = from to to
        val zone = ZoneId.systemDefault()
        val f = from.atStartOfDay(zone).toInstant()
        val t = to.atStartOfDay(zone).toInstant()
        load({ calEvents }, { calEvents = it }) { api.get<CalEvents>("/api/workspaces/$w/calendars/events?from=$f&to=$t") }
    }

    private fun reloadEvents() { calRange?.let { (f, t) -> loadEvents(f, t) }; loadUpcoming() }

    fun toggleCalendar(id: String) {
        hiddenCalendars = if (id in hiddenCalendars) hiddenCalendars - id else hiddenCalendars + id
        prefs.edit().putStringSet("hiddenCalendars", hiddenCalendars).apply()
    }

    suspend fun eventDetail(id: String): EventDetail? = ws?.let { w -> runCatching { api.get<EventDetail>("/api/workspaces/$w/events/$id") }.getOrNull() }

    /** Création ou modification d'un événement de l'espace (corps comme l'interface web). */
    suspend fun saveEvent(id: String?, body: JsonObject): Result<Unit> = runCatching {
        val w = ws ?: throw ApiException(0, "no_workspace")
        if (id == null) api.send("POST", "/api/workspaces/$w/events", body) else api.send("PUT", "/api/workspaces/$w/events/$id", body)
        reloadEvents()
    }

    /** Supprime l'événement, ou une seule occurrence d'une série. */
    fun deleteEvent(e: CalEvent, onlyThis: Boolean) {
        val w = ws ?: return
        val id = e.event_id ?: return
        calEvents = calEvents.copy(data = calEvents.data?.let { d -> d.copy(events = d.events.filter { if (onlyThis) it.id != e.id else it.event_id != id }) })
        viewModelScope.launch {
            val q = if (onlyThis && e.recurring) "?occurrence=${java.net.URLEncoder.encode(e.starts_at, "UTF-8")}" else ""
            runCatching { api.send("DELETE", "/api/workspaces/$w/events/$id$q") }.onFailure { lastError = (it as? ApiException)?.code ?: "offline" }
            reloadEvents()
        }
    }

    // -----------------------------------------------------------------------
    // Mail en lecture seule (G3)
    // -----------------------------------------------------------------------

    var mailAccounts by mutableStateOf(Loadable<List<MailAccount>>()); private set
    var mailAccount by mutableStateOf(prefs.getString("mailAccount", null)); private set
    var mailUnreadOnly by mutableStateOf(false); private set
    var mailList by mutableStateOf(Loadable<List<MailSummary>>()); private set
    var mailHasMore by mutableStateOf(false); private set
    var mailLoadingMore by mutableStateOf(false); private set

    val currentMailAccount: MailAccount? get() = mailAccounts.data?.let { l -> l.firstOrNull { it.id == mailAccount } ?: l.firstOrNull() }
    val mailUnread: Int get() = mailAccounts.data?.sumOf { it.unread } ?: 0

    fun loadMailAccounts() {
        load({ mailAccounts }, { mailAccounts = it }) { api.get<List<MailAccount>>("/api/mail/accounts") }
    }

    fun selectMailAccount(id: String) {
        mailAccount = id
        prefs.edit().putString("mailAccount", id).apply()
        loadMail()
    }

    fun setMailFilter(unread: Boolean) { mailUnreadOnly = unread; loadMail() }

    fun loadMail() {
        val acc = currentMailAccount?.id ?: return
        load({ mailList }, { mailList = it }) {
            val page = api.get<MailPage>("/api/mail/accounts/$acc/messages${if (mailUnreadOnly) "?unread=true" else ""}")
            mailHasMore = page.has_more
            page.messages
        }
    }

    fun loadMoreMail() {
        val acc = currentMailAccount?.id ?: return
        val last = mailList.data?.lastOrNull()?.uid ?: return
        if (mailLoadingMore) return
        mailLoadingMore = true
        viewModelScope.launch {
            runCatching { api.get<MailPage>("/api/mail/accounts/$acc/messages?before=$last${if (mailUnreadOnly) "&unread=true" else ""}") }.onSuccess { p ->
                mailList = mailList.copy(data = mailList.data.orEmpty() + p.messages)
                mailHasMore = p.has_more
            }
            mailLoadingMore = false
        }
    }

    /** Relève la boîte maintenant (serveur IMAP), puis recharge. */
    fun refreshMail() {
        val acc = currentMailAccount?.id ?: return
        mailList = mailList.copy(loading = true)
        viewModelScope.launch {
            runCatching { api.send("POST", "/api/mail/accounts/$acc/refresh") }
            loadMailAccounts(); loadMail()
        }
    }

    private fun localSeen(uid: Long, seen: Boolean) {
        val was = mailList.data?.firstOrNull { it.uid == uid }?.seen ?: return
        mailList = mailList.copy(data = mailList.data?.map { if (it.uid == uid) it.copy(seen = seen) else it })
        if (was != seen) mailAccounts = mailAccounts.copy(data = mailAccounts.data?.map { if (it.id == currentMailAccount?.id) it.copy(unread = (it.unread + if (seen) -1 else 1).coerceAtLeast(0)) else it })
    }

    fun setSeen(uid: Long, seen: Boolean) {
        val acc = currentMailAccount?.id ?: return
        localSeen(uid, seen)
        viewModelScope.launch {
            runCatching { api.send("PUT", "/api/mail/accounts/$acc/messages/$uid/seen", buildJsonObject { put("seen", seen) }) }.onFailure { localSeen(uid, !seen) }
        }
    }

    fun readAllMail() {
        val acc = currentMailAccount?.id ?: return
        viewModelScope.launch { runCatching { api.send("POST", "/api/mail/accounts/$acc/read-all") }; loadMailAccounts(); loadMail() }
    }

    /** Lecture d'un message (marqué lu par le serveur). */
    suspend fun readMail(uid: Long): MailMessage? {
        val acc = currentMailAccount?.id ?: return null
        localSeen(uid, true)
        return runCatching { api.get<MailMessage>("/api/mail/accounts/$acc/messages/$uid") }.getOrNull()
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
