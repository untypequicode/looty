package io.github.untypequicode.looty

import androidx.glance.appwidget.updateAll
import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import io.github.untypequicode.looty.api.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import io.github.untypequicode.looty.offline.*
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.add
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import io.github.untypequicode.looty.push.Push
import io.github.untypequicode.looty.push.PushStore

/** État d'un chargement : données (éventuellement anciennes) + chargement + erreur. */
/** Compte et espaces gardés pour ouvrir l'application sans serveur. */
@Serializable data class CachedSession(val user: User, val workspaces: List<Workspace>)

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
    /** Lien (« /w/<espace>/… ») à ouvrir dès que la session est prête : notification, « Ouvrir dans l'application ». */
    var pendingLink by mutableStateOf<String?>(null)
    private val prefs = app.getSharedPreferences("looty", 0)

    // -----------------------------------------------------------------------
    // Hors ligne : les écrans s'affichent depuis une copie gardée sur l'appareil ; les
    // modifications faites sans serveur sont mises en file d'attente puis rejouées dans l'ordre.
    // -----------------------------------------------------------------------

    val cache = OfflineCache(app)
    private val webCache = io.github.untypequicode.looty.offline.WebCache(app, api)
    private val outbox = Outbox.get(app, api.json)
    /** Le dernier appel n'a pas atteint le serveur : données gardées affichées, modifications en attente. */
    var offline by mutableStateOf(false); private set
    /** Modifications faites hors ligne, pas encore envoyées. */
    var pendingChanges by mutableStateOf(outbox.size()); private set
    /** Modifications hors ligne refusées par le serveur au rejeu (à signaler une fois). */
    var refused by mutableStateOf<List<String>>(emptyList())
    private var network: android.net.ConnectivityManager.NetworkCallback? = null

    private fun markOffline() { if (!offline) offline = true }
    private fun markOnline() { if (offline) offline = false }

    init {
        cache.trim()
        if (outbox.size() > 0) OutboxWorker.schedule(app)
        // Refus constatés par la tâche de fond pendant que l'application était fermée.
        prefs.getStringSet("refusedChanges", null)?.takeIf { it.isNotEmpty() }?.let { refused = refused + it; prefs.edit().remove("refusedChanges").apply() }
        viewModelScope.launch {
            withContext(Dispatchers.IO) { cache.get("taskboard") }?.let { raw ->
                runCatching { api.json.decodeFromString<Map<String, String>>(raw) }.onSuccess { taskBoard.putAll(it) }
            }
        }
        // Dès que le réseau revient : envoi des modifications en attente, puis rechargement.
        val cm = app.getSystemService(android.net.ConnectivityManager::class.java)
        network = object : android.net.ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: android.net.Network) { viewModelScope.launch { retryNow() } }
        }
        runCatching { cm.registerDefaultNetworkCallback(network!!) }
        // Serveur injoignable alors que le réseau est là : nouvel essai toutes les 30 secondes.
        viewModelScope.launch { while (true) { delay(30_000); if (offline || pendingChanges > 0) retryNow() } }
    }

    override fun onCleared() {
        network?.let { n -> runCatching { getApplication<Application>().getSystemService(android.net.ConnectivityManager::class.java).unregisterNetworkCallback(n) } }
    }

    /** Renvoie les modifications en attente, puis recharge les données. */
    suspend fun retryNow() {
        when (session) {
            is Session.Ready -> { drain(); if (offline) refreshAll() }
            is Session.Offline -> start()
            else -> {}
        }
    }

    /** Écrit les modifications en attente dans l'ordre ; s'arrête dès que le serveur est de nouveau injoignable. */
    suspend fun drain() {
        if (!Outbox.replayLock.tryLock()) return
        try {
            val r = outbox.replay(api)
            if (r.offline) markOffline() else if (r.sent) markOnline()
            if (r.refused.isNotEmpty()) refused = refused + r.refused
            pendingChanges = outbox.size()
            if (r.sent) refreshAll()
        } finally {
            Outbox.replayLock.unlock()
        }
    }

    /**
     * Écriture : envoyée tout de suite ; si le serveur est injoignable, mise en file d'attente (rejouée
     * dès que possible). Renvoie true quand elle est en attente. Les écritures déjà en attente passent d'abord.
     */
    private suspend fun write(method: String, path: String, body: JsonElement? = null, label: String = path.substringAfterLast('/')): Boolean {
        if (outbox.size() > 0) drain()
        if (outbox.size() == 0) {
            try { api.send(method, path, body); markOnline(); return false }
            catch (e: Exception) { if (!e.isOffline()) throw e; markOffline() }
        }
        outbox.add(Pending(java.util.UUID.randomUUID().toString(), method, path, body?.toString(), label))
        pendingChanges = outbox.size()
        OutboxWorker.schedule(getApplication())
        return true
    }

    /** Lecture avec copie locale : affiche la copie tout de suite, la remplace par la réponse du serveur. */
    private inline fun <reified T> fetchCached(key: String, crossinline apply: (T) -> Unit, crossinline fetch: suspend () -> T) {
        viewModelScope.launch {
            withContext(Dispatchers.IO) { cache.get(key) }?.let { raw -> runCatching { apply(api.json.decodeFromString<T>(raw)) } }
            try {
                val v = fetch()
                apply(v); markOnline()
                withContext(Dispatchers.IO) { cache.put(key, api.json.encodeToString(v)) }
            } catch (e: Exception) {
                if (e.isOffline()) markOffline()
            }
        }
    }

    private fun persistTaskBoard() {
        val snapshot = taskBoard.toMap()
        viewModelScope.launch(Dispatchers.IO) { cache.put("taskboard", api.json.encodeToString(snapshot)) }
    }

    var session by mutableStateOf<Session>(Session.Loading); private set
    var members by mutableStateOf<Map<String, Member>>(emptyMap()); private set
    /** Groupes de membres de l’espace (mentions, ajout groupé des assignés). */
    var groups by mutableStateOf<List<MemberGroup>>(emptyList()); private set
    var overview by mutableStateOf(Loadable<Overview>()); private set
    var tasks by mutableStateOf(Loadable<List<MyTask>>()); private set
    var taskScope by mutableStateOf(prefs.getString("taskScope", "mine") ?: "mine"); private set
    var pages by mutableStateOf(Loadable<List<Page>>()); private set
    var inbox by mutableStateOf(Loadable<List<Notification>>()); private set
    var upcoming by mutableStateOf(Loadable<List<CalEvent>>()); private set
    var boards by mutableStateOf<List<BoardSummary>>(emptyList()); private set
    /** Droits du rôle dans l’espace ; inconnus (pas encore chargés, hors ligne) = tout permis, le serveur tranche. */
    var access by mutableStateOf<MyAccess?>(null); private set

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
                    if (wsId == null) Session.Offline("no_workspace") else {
                        withContext(Dispatchers.IO) { cache.put("session", api.json.encodeToString(CachedSession(user, list))) }
                        markOnline()
                        Session.Ready(user, list, wsId)
                    }
                }
            } catch (e: ApiException) {
                if (e.status == 401) Session.NeedLogin else offlineSession(e) ?: Session.Offline(e.code)
            } catch (e: Exception) {
                offlineSession(e) ?: Session.Offline(e.message ?: "offline")
            }
            if (session is Session.Ready) refreshAll()
        }
    }

    /** Serveur injoignable : si on s'est déjà connecté ici, l'application s'ouvre sur les données gardées. */
    private suspend fun offlineSession(e: Throwable): Session.Ready? {
        if (!e.isOffline()) return null
        val c = withContext(Dispatchers.IO) { cache.get("session") }?.let { runCatching { api.json.decodeFromString<CachedSession>(it) }.getOrNull() } ?: return null
        val saved = prefs.getString("workspace", null)
        val wsId = c.workspaces.firstOrNull { it.id == saved }?.id ?: c.workspaces.firstOrNull()?.id ?: return null
        markOffline()
        return Session.Ready(c.user, c.workspaces, wsId)
    }

    fun switchWorkspace(id: String) {
        val s = session as? Session.Ready ?: return
        prefs.edit().putString("workspace", id).apply()
        session = s.copy(workspaceId = id)
        access = null
        overview = Loadable(); tasks = Loadable(); pages = Loadable(); inbox = Loadable(); upcoming = Loadable()
        refreshAll()
    }

    fun refreshAll() {
        prefetchDocs()
        loadAccess(); loadMembers(); loadOverview(); loadTasks(); loadPages(); loadDocCommentCounts(); loadInbox(); loadUpcoming(); loadBoards(); loadMailAccounts()
    }

    /** Charge une liste : copie gardée affichée d'abord, puis réponse du serveur ; hors ligne, la copie reste. */
    private inline fun <reified T> load(crossinline get: () -> Loadable<T>, crossinline set: (Loadable<T>) -> Unit, key: String?, crossinline block: suspend () -> T) {
        set(get().copy(loading = true, error = null))
        viewModelScope.launch {
            if (key != null && get().data == null) {
                withContext(Dispatchers.IO) { cache.get(key) }?.let { raw -> runCatching { api.json.decodeFromString<T>(raw) }.getOrNull() }?.let { set(Loadable(it, loading = true)) }
            }
            try {
                val v = block()
                set(Loadable(v)); markOnline()
                if (key != null) withContext(Dispatchers.IO) { cache.put(key, api.json.encodeToString(v)) }
            } catch (e: Exception) {
                val off = e.isOffline()
                if (off) markOffline()
                val cur = get()
                set(cur.copy(loading = false, error = if (cur.data != null && off) null else (e as? ApiException)?.code ?: "offline"))
            }
        }
    }

    fun loadAccess() { val w = ws ?: return; fetchCached("${w}/access", { a: MyAccess -> access = a }) { api.get<MyAccess>("/api/workspaces/$w/access/me") } }
    private fun level(name: String?) = when (name) { "none" -> 0; "read" -> 1; "comment" -> 2; "validate" -> 3; "edit" -> 4; "manage" -> 5; else -> 4 }
    private fun allows(area: (Permissions) -> String, needed: Int) = access?.permissions?.let { level(area(it)) >= needed } ?: true
    /** Créer, modifier, déplacer, supprimer des tâches. */
    val canEditBoards: Boolean get() = allows({ it.boards }, 4)
    /** Terminer / rouvrir une tâche (reste possible en lecture selon le rôle). */
    val canValidate: Boolean get() = allows({ it.boards }, 3)
    val canComment: Boolean get() = allows({ it.boards }, 2)
    /** Créer et organiser les tableaux : colonnes, étiquettes, automatisations. */
    val canManageBoards: Boolean get() = allows({ it.boards }, 5)
    val isAdmin: Boolean get() = access?.permissions?.admin ?: false
    val canEditDocs: Boolean get() = allows({ it.docs }, 4)
    val canEditCalendar: Boolean get() = allows({ it.calendar }, 4)
    /** Document complet (titre + HTML) pour la lecture native ; null si introuvable. */
    suspend fun pageDoc(id: String): Page? = ws?.let { w ->
        runCatching { api.get<Page>("/api/workspaces/$w/pages/$id").also { p -> markOnline(); withContext(Dispatchers.IO) { cache.put("page-$id", api.json.encodeToString(p)) } } }.onFailure { if (it.isOffline()) markOffline() }.getOrNull()
            ?: withContext(Dispatchers.IO) { cache.get("page-$id") }?.let { raw -> runCatching { api.json.decodeFromString<Page>(raw) }.getOrNull() }
    }
    fun loadMembers() {
        val w = ws ?: return
        fetchCached("${w}/members", { m: Members -> members = m.members.associateBy { it.user_id } }) { api.get<Members>("/api/workspaces/$w/members") }
        fetchCached("${w}/groups", { g: List<MemberGroup> -> groups = g }) { api.get<List<MemberGroup>>("/api/workspaces/$w/groups") }
    }
    fun loadOverview() { val w = ws ?: return; load({ overview }, { overview = it }, "${w}/overview") { api.get<Overview>("/api/workspaces/$w/overview") } }
    fun loadTasks() {
        val w = ws ?: return
        load({ tasks }, { tasks = it }, "${w}/tasks-${taskScope}") {
            api.get<MyTasks>("/api/workspaces/$w/my_tasks?scope=$taskScope").tasks.also { l -> l.forEach { taskBoard[it.id] = it.board_id }; persistTaskBoard() }
        }
        // Le widget de l’écran d’accueil suit les tâches affichées.
        viewModelScope.launch { runCatching { io.github.untypequicode.looty.widget.LootyWidget().updateAll(getApplication()) } }
    }
    fun loadPages() { val w = ws ?: return; load({ pages }, { pages = it }, "${w}/pages") { api.get<List<Page>>("/api/workspaces/$w/pages") } }
    fun loadInbox() { val w = ws ?: return; load({ inbox }, { inbox = it }, "${w}/inbox") { api.get<List<Notification>>("/api/notifications?workspace_id=$w") } }
    fun loadBoards() { val w = ws ?: return; fetchCached("${w}/boards", { l: List<BoardSummary> -> boards = l }) { api.get<List<BoardSummary>>("/api/workspaces/$w/boards") } }
    /** Événements d'aujourd'hui et de demain (calendriers de l'espace et personnels). */
    fun loadUpcoming() {
        val w = ws ?: return
        val zone = ZoneId.systemDefault()
        val from = LocalDate.now().atStartOfDay(zone).toInstant()
        val to = from.plusSeconds(2 * 86400)
        load({ upcoming }, { upcoming = it }, "${w}/upcoming") {
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
        // Copie locale tout de suite (visible aussi hors ligne), puis envoi ou mise en file d'attente.
        tasks = tasks.copy(data = tasks.data?.map { if (it.id == id) { if (it.is_per_person) it.copy(done_by_me = !it.done_by_me) else it.copy(is_completed = !it.is_completed) } else it })
        saveTasksCache()
        viewModelScope.launch {
            runCatching { write("PUT", "/api/boards/tasks/$id/toggle_validation", null, tasks.data?.firstOrNull { it.id == id }?.title ?: "task") }
            // Laisse voir la coche avant que la tâche quitte la liste.
            kotlinx.coroutines.delay(900)
            loadTasks(); loadOverview(); onDone()
        }
    }

    private fun saveTasksCache() {
        val w = ws ?: return
        val data = tasks.data ?: return
        val key = "${w}/tasks-${taskScope}"
        viewModelScope.launch(Dispatchers.IO) { cache.put(key, api.json.encodeToString(data)) }
    }

    fun markRead(n: Notification) {
        if (n.read_at != null) return
        inbox = inbox.copy(data = inbox.data?.map { if (it.id == n.id) it.copy(read_at = Instant.now().toString()) else it })
        viewModelScope.launch { runCatching { write("PUT", "/api/notifications/${n.id}/read", buildJsonObject { put("read", true) }, "notification") } }
    }

    fun markAllRead() {
        val w = ws ?: return
        inbox = inbox.copy(data = inbox.data?.map { it.copy(read_at = it.read_at ?: Instant.now().toString()) })
        viewModelScope.launch { runCatching { write("POST", "/api/notifications/read-all?workspace_id=$w", null, "inbox") }; loadInbox() }
    }

    /** Ajout rapide : première colonne du tableau choisi. */
    /** Colonnes d'un tableau : celles du serveur, sinon la copie gardée sur l'appareil. */
    private suspend fun boardColumns(boardId: String): Board =
        try { api.get<Board>("/api/boards/$boardId").also { markOnline() } }
        catch (e: Exception) {
            if (!e.isOffline()) throw e
            markOffline()
            val b = boardData[boardId]?.data ?: withContext(Dispatchers.IO) { cache.get("board-$boardId") }?.let { api.json.decodeFromString<BoardFull>(it) } ?: throw e
            Board(b.id, b.name, b.columns.map { Column(it.id, it.name, it.position) })
        }

    /** Tâche créée hors ligne : visible tout de suite dans le tableau et les listes. */
    private fun insertLocalTask(boardId: String, boardName: String, column: Column, t: Task) {
        taskBoard[t.id] = boardId
        persistTaskBoard()
        patchLocal(boardId) { bd -> bd.copy(columns = bd.columns.map { c -> if (c.id == column.id) c.copy(tasks = listOf(t) + c.tasks) else c }) }
        tasks = tasks.copy(data = listOf(MyTask(id = t.id, title = t.title, due_date = t.due_date, board_id = boardId, board_name = boardName, column_name = column.name, assignees = t.assignees)) + tasks.data.orEmpty())
        saveTasksCache()
    }

    suspend fun createTask(boardId: String, title: String, due: LocalDate?, assignMe: Boolean, description: String? = null): Result<String> = runCatching {
        val board = boardColumns(boardId)
        val col = board.columns.minByOrNull { it.position } ?: throw ApiException(0, "no_column")
        val id = java.util.UUID.randomUUID().toString()
        val dueIso = due?.atTime(12, 0)?.atZone(ZoneId.systemDefault())?.toInstant()?.toString()
        val body = buildJsonObject {
            put("id", id)
            put("title", title.trim())
            put("at_top", true)
            if (description != null) put("description", description)
            if (due != null) put("due_date", due.atTime(12, 0).atZone(ZoneId.systemDefault()).toInstant().toString())
            if (assignMe) me?.let { putJsonArray("assignees") { add(it.id) } }
        }
        prefs.edit().putString("lastBoard", boardId).apply()
        val queued = write("POST", "/api/boards/columns/${col.id}/tasks", body, title.trim())
        if (queued) insertLocalTask(boardId, board.name, col, Task(id = id, column_id = col.id, title = title.trim(), description = description, due_date = dueIso, assignees = if (assignMe) listOfNotNull(me?.id) else emptyList(), position = 0.0))
        else { loadTasks(); loadOverview() }
        id
    }
    val lastBoard: String? get() = prefs.getString("lastBoard", null)

    /** Joint des fichiers (partage depuis une autre application) à une tâche. Renvoie le nombre d'échecs. */
    suspend fun attachFiles(taskId: String, files: List<SharedFile>): Int {
        var failed = 0
        for (f in files) runCatching { upload("/api/boards/tasks/$taskId/attachments", f) }.onFailure { failed++; android.util.Log.w("LootyShare", "upload failed: ${f.name}", it) }
        return failed
    }

    /** Envoi d'un fichier ; hors ligne, il est gardé sur l'appareil et envoyé avec les autres modifications. */
    private suspend fun upload(path: String, f: SharedFile) {
        if (outbox.size() > 0) drain()
        if (outbox.size() == 0) {
            try { api.upload(path, f.name, f.mime, f.bytes); markOnline(); return }
            catch (e: Exception) { if (!e.isOffline()) throw e; markOffline() }
        }
        val id = java.util.UUID.randomUUID().toString()
        withContext(Dispatchers.IO) { outbox.files.mkdirs(); java.io.File(outbox.files, id).writeBytes(f.bytes) }
        outbox.add(Pending(id, "UPLOAD", path, buildJsonObject { put("name", f.name); put("mime", f.mime) }.toString(), f.name))
        pendingChanges = outbox.size()
        OutboxWorker.schedule(getApplication())
    }

    private var lastPrefetch = 0L

    /**
     * Préchargement pour le hors ligne, au plus toutes les 10 minutes : tous les tableaux, puis les favoris et
     * derniers documents modifiés avec leurs images (ces derniers pas sur une connexion limitée).
     */
    fun prefetchDocs() {
        val now = System.currentTimeMillis()
        if (now - lastPrefetch < 10 * 60_000) return
        lastPrefetch = now
        viewModelScope.launch {
            delay(4_000)
            if (offline) return@launch
            // Tableaux d'abord (petits) : l'ajout rapide et l'ouverture d'une tâche marchent alors hors ligne.
            for (b in boards) {
                runCatching { api.get<BoardFull>("/api/boards/${b.id}") }.onSuccess { full ->
                    full.allTasks.forEach { taskBoard[it.id] = b.id }
                    withContext(Dispatchers.IO) { cache.put("board-${b.id}", api.json.encodeToString(full)) }
                }
            }
            persistTaskBoard()
            // Interface web complète (éditeur, Moodle…) puis documents : pas sur une connexion limitée.
            val cm = getApplication<Application>().getSystemService(android.net.ConnectivityManager::class.java)
            if (cm.isActiveNetworkMetered) return@launch
            withContext(Dispatchers.IO) { webCache.prefetchAssets() }
            val list = pages.data ?: return@launch
            val wanted = (list.filter { it.is_favorite == true } + list.sortedByDescending { it.updated_at ?: "" }.take(15)).distinctBy { it.id }.take(25)
            for (p in wanted) {
                val cached = withContext(Dispatchers.IO) { cache.get("page-${p.id}") }?.let { runCatching { api.json.decodeFromString<Page>(it) }.getOrNull() }
                val doc = if (cached != null && cached.updated_at == p.updated_at) cached else pageDoc(p.id) ?: continue
                Regex("src=\"(/uploads/[^\"]+)\"").findAll(doc.content.orEmpty()).take(10).forEach { m ->
                    val path = m.groupValues[1]
                    if (withContext(Dispatchers.IO) { cache.getBytes("img-${path}") } == null) {
                        runCatching { api.bytes(path) }.onSuccess { b -> withContext(Dispatchers.IO) { cache.putBytes("img-${path}", b) } }
                    }
                }
            }
        }
    }

    suspend fun createPage(title: String = "", content: String? = null, parentId: String? = null): Result<String> = runCatching {
        val w = ws ?: throw ApiException(0, "no_workspace")
        val id = java.util.UUID.randomUUID().toString()
        val queued = write("POST", "/api/workspaces/$w/pages", buildJsonObject { put("id", id); put("title", title); if (content != null) put("content", content); if (parentId != null) put("parent_id", parentId) }, title.ifBlank { "document" })
        if (!queued) loadPages()
        id
    }

    // -----------------------------------------------------------------------
    // Documents (N3) : organisation, partage, commentaires, versions. La
    // modification du contenu reste dans l'éditeur collaboratif du site.
    // -----------------------------------------------------------------------

    /** Commentaires non résolus par document. */
    var docCommentCounts by mutableStateOf<Map<String, Int>>(emptyMap()); private set

    fun loadDocCommentCounts() {
        val w = ws ?: return
        viewModelScope.launch { runCatching { api.get<Map<String, Int>>("/api/workspaces/$w/comments/counts?kind=page") }.onSuccess { docCommentCounts = it } }
    }

    private fun patchPage(id: String, f: (Page) -> Page) { pages = pages.copy(data = pages.data?.map { if (it.id == id) f(it) else it }) }

    /** Modification d'un document (titre, icône, emplacement) : appliquée tout de suite, envoyée, puis la liste rechargée. */
    fun updatePage(id: String, body: JsonObject, label: String, local: (Page) -> Page) {
        val w = ws ?: return
        patchPage(id, local)
        viewModelScope.launch {
            runCatching { write("PUT", "/api/workspaces/$w/pages/$id", body, label) }.onFailure(::fail)
            loadPages()
        }
    }

    fun renamePage(p: Page, title: String) = updatePage(p.id, buildJsonObject { put("title", title) }, title) { it.copy(title = title) }
    fun setPageIcon(p: Page, icon: String?) = updatePage(p.id, buildJsonObject { put("icon", icon) }, p.title) { it.copy(icon = icon) }

    /** Déplace un document sous un autre (ou à la racine), à la fin de ses nouvelles sœurs. */
    fun movePage(p: Page, parentId: String?) {
        val pos = (pages.data.orEmpty().filter { it.parent_id == parentId && it.id != p.id }.maxOfOrNull { it.position } ?: 0.0) + 65536
        updatePage(p.id, buildJsonObject { put("parent_id", parentId); put("position", pos) }, p.title) { it.copy(parent_id = parentId, position = pos) }
    }

    fun toggleFavorite(p: Page) {
        val w = ws ?: return
        patchPage(p.id) { it.copy(is_favorite = it.is_favorite != true) }
        viewModelScope.launch { runCatching { write("POST", "/api/workspaces/$w/pages/${p.id}/favorite", null, p.title) }.onFailure(::fail); loadPages() }
    }

    /** Ouverture d'un document (section « Récents »). */
    fun recordView(id: String) {
        val w = ws ?: return
        viewModelScope.launch { runCatching { api.send("POST", "/api/workspaces/$w/pages/$id/view") } }
    }

    /** Document (et sous-pages) placé dans la corbeille ; [restorePage] l'en sort. */
    suspend fun deletePage(p: Page): Boolean {
        val w = ws ?: return false
        val gone = buildSet { add(p.id); var added = true; while (added) { added = false; pages.data.orEmpty().forEach { if (it.parent_id in this && add(it.id)) added = true } } }
        val before = pages
        pages = pages.copy(data = pages.data?.filter { it.id !in gone })
        return runCatching { api.send("DELETE", "/api/workspaces/$w/pages/${p.id}") }.onFailure { pages = before; fail(it) }.isSuccess.also { loadPages() }
    }

    fun restorePage(id: String) {
        viewModelScope.launch { runCatching { api.send("POST", "/api/trash/pages/$id/restore") }.onFailure(::fail); loadPages() }
    }

    /** Lien public : activé (renvoie le document avec son jeton) ou désactivé. */
    suspend fun setPagePublic(id: String, on: Boolean): Page? {
        val w = ws ?: return null
        return runCatching { if (on) api.post<Page>("/api/workspaces/$w/pages/$id/share") else api.json.decodeFromString<Page>(api.raw("DELETE", "/api/workspaces/$w/pages/$id/share")) }
            .onFailure(::fail).getOrNull()?.also { r -> patchPage(id) { it.copy(share_token = r.share_token, is_public = r.is_public) } }
    }

    suspend fun pagePermissions(id: String): List<PagePerm>? = ws?.let { w -> runCatching { api.get<List<PagePerm>>("/api/workspaces/$w/pages/$id/permissions") }.onFailure(::fail).getOrNull() }

    suspend fun setPagePermission(id: String, userId: String, role: String, inherit: Boolean, allowSubpages: Boolean): Boolean {
        val w = ws ?: return false
        return runCatching {
            api.send("PUT", "/api/workspaces/$w/pages/$id/permissions", buildJsonObject { put("user_id", userId); put("role", role); put("inherit", inherit); put("allow_subpages", allowSubpages) })
        }.onFailure(::fail).isSuccess
    }

    // Commentaires d'un document
    suspend fun docComments(id: String): List<DocComment>? = ws?.let { w ->
        runCatching { api.get<List<DocComment>>("/api/workspaces/$w/comments?kind=page&id=$id") }.onFailure(::fail).getOrNull()
    }

    suspend fun addDocComment(pageId: String, text: String, parentId: String?, mentions: List<io.github.untypequicode.looty.ui.Html.Mention> = emptyList()): Boolean {
        val w = ws ?: return false
        return runCatching {
            api.send("POST", "/api/workspaces/$w/comments", buildJsonObject {
                put("kind", "page"); put("id", pageId); put("content", io.github.untypequicode.looty.ui.Html.fromPlain(text, mentions)); parentId?.let { put("parent_id", it) }
            })
        }.onFailure(::fail).isSuccess.also { if (it) loadDocCommentCounts() }
    }

    suspend fun editDocComment(id: String, text: String, mentions: List<io.github.untypequicode.looty.ui.Html.Mention> = emptyList()): Boolean =
        runCatching { api.send("PUT", "/api/comments/$id", buildJsonObject { put("content", io.github.untypequicode.looty.ui.Html.fromPlain(text, mentions)) }) }.onFailure(::fail).isSuccess

    suspend fun deleteDocComment(id: String): Boolean =
        runCatching { api.send("DELETE", "/api/comments/$id") }.onFailure(::fail).isSuccess.also { if (it) loadDocCommentCounts() }

    suspend fun resolveDocComment(id: String, resolved: Boolean): Boolean =
        runCatching { api.send("POST", "/api/comments/$id/resolve", buildJsonObject { put("resolved", resolved) }) }.onFailure(::fail).isSuccess.also { if (it) loadDocCommentCounts() }

    // Versions
    suspend fun revisions(pageId: String): List<Revision>? = runCatching { api.get<List<Revision>>("/api/pages/$pageId/revisions") }.onFailure(::fail).getOrNull()

    suspend fun createRevision(pageId: String, label: String?): Boolean =
        runCatching { api.send("POST", "/api/pages/$pageId/revisions", buildJsonObject { put("kind", "manual"); put("label", label) }) }.onFailure(::fail).isSuccess

    suspend fun renameRevision(pageId: String, revId: String, label: String?): Boolean =
        runCatching { api.send("PATCH", "/api/pages/$pageId/revisions/$revId", buildJsonObject { put("label", label) }) }.onFailure(::fail).isSuccess

    suspend fun deleteRevision(pageId: String, revId: String): Boolean =
        runCatching { api.send("DELETE", "/api/pages/$pageId/revisions/$revId") }.onFailure(::fail).isSuccess

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
            if (boardData[id]?.data == null) {
                withContext(Dispatchers.IO) { cache.get("board-$id") }?.let { raw -> runCatching { api.json.decodeFromString<BoardFull>(raw) }.getOrNull() }?.let { b ->
                    b.allTasks.forEach { taskBoard[it.id] = id }
                    boardData[id] = Loadable(b, loading = true)
                }
            }
            boardData[id] = try {
                val b = api.get<BoardFull>("/api/boards/$id")
                b.allTasks.forEach { taskBoard[it.id] = id }
                markOnline()
                withContext(Dispatchers.IO) { cache.put("board-$id", api.json.encodeToString(b)) }
                Loadable(b)
            } catch (e: Exception) {
                val off = e.isOffline()
                if (off) markOffline()
                val last = boardData[id] ?: Loadable()
                last.copy(loading = false, error = if (last.data != null && off) null else (e as? ApiException)?.code ?: "offline")
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
        val next = f(cur)
        boardData[boardId] = entry.copy(data = next)
        // La copie gardée suit les modifications locales (visibles aussi après un redémarrage hors ligne).
        viewModelScope.launch(Dispatchers.IO) { cache.put("board-$boardId", api.json.encodeToString(next)) }
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
        mutate(b, { mapTask(it, task.id, local) }) { write("PUT", "/api/boards/tasks/${task.id}", body, task.title) }
    }

    fun toggleDone(task: Task) {
        val b = taskBoard[task.id] ?: return
        val meId = me?.id ?: return
        mutate(b, { bd -> mapTask(bd, task.id) { t ->
            if (t.is_per_person) t.copy(validations = if (meId in t.validations) t.validations - meId else t.validations + meId)
            else t.copy(is_completed = !t.is_completed)
        } }) { write("PUT", "/api/boards/tasks/${task.id}/toggle_validation", null, task.title) }
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
        }) { write("PUT", "/api/boards/tasks/${task.id}/move", buildJsonObject { put("column_id", columnId); put("position", pos) }, task.title) }
    }

    fun setLabel(task: Task, label: Label, on: Boolean) {
        val b = taskBoard[task.id] ?: return
        mutate(b, { bd -> mapTask(bd, task.id) { t -> t.copy(labels = if (on) t.labels + label else t.labels.filter { it.id != label.id }) } }) {
            write(if (on) "POST" else "DELETE", "/api/boards/tasks/${task.id}/labels/${label.id}", null, task.title)
        }
    }

    fun setAssignee(task: Task, userId: String, on: Boolean) {
        val b = taskBoard[task.id] ?: return
        mutate(b, { bd -> mapTask(bd, task.id) { t -> t.copy(assignees = if (on) t.assignees + userId else t.assignees - userId) } }) {
            write(if (on) "POST" else "DELETE", "/api/boards/tasks/${task.id}/assignees/$userId", null, task.title)
        }
    }

    fun addSubtask(parent: Task, title: String) {
        val b = taskBoard[parent.id] ?: return
        viewModelScope.launch {
            runCatching { write("POST", "/api/boards/tasks/${parent.id}/subtasks", buildJsonObject { put("title", title.trim()) }, title.trim()) }
                .onFailure { lastError = (it as? ApiException)?.code ?: "offline" }
            loadBoard(b)
        }
    }

    suspend fun createInColumn(boardId: String, columnId: String, title: String): Result<String> = runCatching {
        val id = java.util.UUID.randomUUID().toString()
        val queued = write("POST", "/api/boards/columns/$columnId/tasks", buildJsonObject { put("id", id); put("title", title.trim()); put("at_top", true) }, title.trim())
        taskBoard[id] = boardId
        if (queued) {
            val col = boardData[boardId]?.data?.column(columnId)
            if (col != null) insertLocalTask(boardId, boardData[boardId]?.data?.name.orEmpty(), Column(col.id, col.name, col.position), Task(id = id, column_id = columnId, title = title.trim()))
        } else loadBoard(boardId)
        id
    }

    fun deleteTask(task: Task) {
        val b = taskBoard[task.id] ?: return
        mutate(b, { bd -> bd.copy(columns = bd.columns.map { c -> c.copy(tasks = c.tasks.filter { it.id != task.id && it.parent_id != task.id }) }) }) {
            write("DELETE", "/api/boards/tasks/${task.id}", null, task.title)
        }
    }

    /** Annulation d'une suppression (corbeille). */
    fun restoreTask(taskId: String) {
        val b = taskBoard[taskId] ?: return
        viewModelScope.launch { runCatching { write("POST", "/api/trash/tasks/$taskId/restore", null, "task") }; loadBoard(b); loadTasks() }
    }

    fun loadComments(taskId: String) {
        viewModelScope.launch {
            // Copie gardée d'abord (ouverture hors ligne), puis le serveur.
            withContext(Dispatchers.IO) { cache.get("comments-$taskId") }?.let { raw -> runCatching { api.json.decodeFromString<List<Comment>>(raw) }.getOrNull() }?.let { if (comments[taskId] == null) comments[taskId] = it }
            withContext(Dispatchers.IO) { cache.get("attachments-$taskId") }?.let { raw -> runCatching { api.json.decodeFromString<List<Attachment>>(raw) }.getOrNull() }?.let { if (attachments[taskId] == null) attachments[taskId] = it }
            runCatching { api.get<List<Comment>>("/api/boards/tasks/$taskId/comments") }.onSuccess { comments[taskId] = it; markOnline(); withContext(Dispatchers.IO) { cache.put("comments-$taskId", api.json.encodeToString(it)) } }.onFailure { if (it.isOffline()) markOffline() }
            runCatching { api.get<List<Attachment>>("/api/boards/tasks/$taskId/attachments") }.onSuccess { attachments[taskId] = it; withContext(Dispatchers.IO) { cache.put("attachments-$taskId", api.json.encodeToString(it)) } }
        }
    }

    suspend fun addComment(taskId: String, text: String, mentions: List<io.github.untypequicode.looty.ui.Html.Mention> = emptyList()): Boolean = runCatching {
        val html = io.github.untypequicode.looty.ui.Html.fromPlain(text, mentions)
        val queued = write("POST", "/api/boards/tasks/$taskId/comments", buildJsonObject { put("content", html) }, "comment")
        // Hors ligne : le commentaire apparaît tout de suite ; il sera envoyé avec le reste.
        if (queued) comments[taskId] = comments[taskId].orEmpty() + Comment("local-" + java.util.UUID.randomUUID(), taskId, me?.id.orEmpty(), html, Instant.now().toString())
        else loadComments(taskId)
    }.isSuccess

    // -----------------------------------------------------------------------
    // Gestion des tableaux, pièces jointes, commentaires et historique (N1)
    // -----------------------------------------------------------------------

    /** Vue d'un tableau : « columns » (une colonne par page) ou « list ». */
    var boardView by mutableStateOf(prefs.getString("boardView", "columns") ?: "columns"); private set
    fun setBoardViewMode(v: String) { boardView = v; prefs.edit().putString("boardView", v).apply() }

    /** Erreur d'une écriture, signalée à l'écran (une annulation, elle, n'est pas une erreur). */
    private fun fail(e: Throwable) { if (e is kotlinx.coroutines.CancellationException) return; lastError = (e as? ApiException)?.code ?: "offline" }

    /** Action qui doit aller au bout même si l'écran ou la feuille qui l'a lancée se ferme. */
    fun launch(block: suspend () -> Unit) { viewModelScope.launch { block() } }

    suspend fun createBoard(name: String): String? = runCatching {
        val w = ws ?: throw ApiException(0, "no_workspace")
        api.post<BoardSummary>("/api/workspaces/$w/boards", buildJsonObject { put("name", name.trim()); put("with_default_columns", true) })
    }.onFailure(::fail).getOrNull()?.also { loadBoards(); loadOverview() }?.id

    fun renameBoard(boardId: String, name: String) =
        mutate(boardId, { it.copy(name = name) }) { write("PUT", "/api/boards/$boardId", buildJsonObject { put("name", name) }, name); loadBoards() }

    fun saveAutomations(boardId: String, a: JsonObject) =
        mutate(boardId, { it.copy(automations = a) }) { write("PUT", "/api/boards/$boardId", buildJsonObject { put("automations", a) }, "automations") }

    /** Suppression définitive d'un tableau et de ses tâches. */
    suspend fun deleteBoard(boardId: String): Boolean = runCatching { api.send("DELETE", "/api/boards/$boardId") }.onFailure(::fail).isSuccess.also { ok ->
        if (ok) { boardData.remove(boardId); loadBoards(); loadOverview(); loadTasks() }
    }

    suspend fun createColumn(boardId: String, name: String): Boolean =
        runCatching { api.send("POST", "/api/boards/$boardId/columns", buildJsonObject { put("name", name.trim()) }) }.onFailure(::fail).isSuccess.also { loadBoard(boardId) }

    fun renameColumn(boardId: String, columnId: String, name: String) =
        mutate(boardId, { b -> b.copy(columns = b.columns.map { if (it.id == columnId) it.copy(name = name) else it }) }) {
            write("PUT", "/api/boards/columns/$columnId", buildJsonObject { put("name", name) }, name)
        }

    /** Déplace une colonne d'un cran vers la gauche (-1) ou la droite (+1). */
    fun moveColumn(boardId: String, columnId: String, delta: Int) {
        val cols = boardData[boardId]?.data?.columns?.sortedBy { it.position } ?: return
        val i = cols.indexOfFirst { it.id == columnId }
        val j = i + delta
        if (i < 0 || j !in cols.indices) return
        val target = cols[j].position
        val beyond = cols.getOrNull(if (delta < 0) j - 1 else j + 1)?.position
        val pos = when {
            beyond != null -> (target + beyond) / 2
            delta < 0 -> target - 65536
            else -> target + 65536
        }
        mutate(boardId, { b -> b.copy(columns = b.columns.map { if (it.id == columnId) it.copy(position = pos) else it }) }) {
            write("PUT", "/api/boards/columns/$columnId/move", buildJsonObject { put("position", pos) }, "column")
        }
    }

    fun deleteColumn(boardId: String, columnId: String) =
        mutate(boardId, { b -> b.copy(columns = b.columns.filter { it.id != columnId }) }) { write("DELETE", "/api/boards/columns/$columnId", null, "column") }

    suspend fun createLabel(boardId: String, name: String, color: String): Label? =
        runCatching { api.post<Label>("/api/boards/$boardId/labels", buildJsonObject { put("name", name.trim()); put("color", color) }) }
            .onFailure(::fail).getOrNull()?.also { l -> patchLocal(boardId) { it.copy(labels = it.labels + l) } }

    fun updateLabel(boardId: String, label: Label) =
        mutate(boardId, { b ->
            b.copy(labels = b.labels.map { if (it.id == label.id) label else it },
                columns = b.columns.map { c -> c.copy(tasks = c.tasks.map { t -> t.copy(labels = t.labels.map { if (it.id == label.id) label else it }) }) })
        }) { write("PUT", "/api/boards/labels/${label.id}", buildJsonObject { put("name", label.name); put("color", label.color) }, label.name) }

    fun deleteLabel(boardId: String, labelId: String) =
        mutate(boardId, { b ->
            b.copy(labels = b.labels.filter { it.id != labelId },
                columns = b.columns.map { c -> c.copy(tasks = c.tasks.map { t -> t.copy(labels = t.labels.filter { it.id != labelId }) }) })
        }) { write("DELETE", "/api/boards/labels/$labelId", null, "label") }

    /** Ajoute des fichiers choisis sur l'appareil ; renvoie le nombre d'échecs. */
    suspend fun addAttachments(taskId: String, files: List<SharedFile>): Int = attachFiles(taskId, files).also { loadComments(taskId); loadActivities(taskId) }

    fun deleteAttachment(taskId: String, a: Attachment) {
        attachments[taskId] = attachments[taskId].orEmpty().filter { it.id != a.id }
        viewModelScope.launch {
            runCatching { write("DELETE", "/api/boards/tasks/$taskId/attachments/${a.id}", null, a.filename) }.onFailure(::fail)
            loadComments(taskId)
        }
    }

    /** Pièce jointe enregistrée dans le cache de l'application (pour l'ouvrir avec une autre application). */
    suspend fun attachmentFile(a: Attachment): java.io.File? = runCatching {
        val dir = java.io.File(getApplication<Application>().cacheDir, "attachments/${a.id}")
        val name = a.filename.replace(Regex("[/\\\\:*?\"<>|]"), "_").ifBlank { "file" }
        val f = java.io.File(dir, name)
        if (!f.exists()) {
            val bytes = api.bytes("/" + a.file_path.trimStart('/'))
            withContext(Dispatchers.IO) { dir.mkdirs(); f.writeBytes(bytes) }
        }
        f
    }.onFailure(::fail).getOrNull()

    fun editComment(taskId: String, c: Comment, text: String) {
        val html = io.github.untypequicode.looty.ui.Html.let { it.fromPlain(text, it.mentionsOf(c.content)) }
        comments[taskId] = comments[taskId].orEmpty().map { if (it.id == c.id) it.copy(content = html, updated_at = Instant.now().toString()) else it }
        viewModelScope.launch {
            runCatching { write("PUT", "/api/boards/tasks/$taskId/comments/${c.id}", buildJsonObject { put("content", html) }, "comment") }.onFailure(::fail)
            loadComments(taskId)
        }
    }

    fun deleteComment(taskId: String, c: Comment) {
        comments[taskId] = comments[taskId].orEmpty().filter { it.id != c.id }
        viewModelScope.launch {
            runCatching { write("DELETE", "/api/boards/tasks/$taskId/comments/${c.id}", null, "comment") }.onFailure(::fail)
            loadComments(taskId)
        }
    }

    val activities = mutableStateMapOf<String, List<TaskActivity>>()

    fun loadActivities(taskId: String) {
        viewModelScope.launch { runCatching { api.get<List<TaskActivity>>("/api/boards/tasks/$taskId/activities") }.onSuccess { activities[taskId] = it } }
    }

    /** Remet le titre ou la description d'avant une modification. */
    fun restoreActivity(taskId: String, a: TaskActivity) {
        viewModelScope.launch {
            runCatching { api.send("POST", "/api/boards/tasks/$taskId/activities/${a.id}/restore") }.onFailure(::fail)
            taskBoard[taskId]?.let(::loadBoard)
            loadActivities(taskId)
        }
    }

    // -----------------------------------------------------------------------
    // Drive et projets (N4). Les gros fichiers passent en flux, sans tout garder en mémoire.
    // -----------------------------------------------------------------------

    private fun enc(s: String) = java.net.URLEncoder.encode(s, "UTF-8")

    /** Fichier choisi sur le téléphone : nom, type, taille et ouverture du contenu. */
    private fun picked(uri: android.net.Uri): Triple<String, String, Long> {
        val cr = getApplication<Application>().contentResolver
        var name = uri.lastPathSegment ?: "file"
        var size = -1L
        runCatching {
            cr.query(uri, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME, android.provider.OpenableColumns.SIZE), null, null, null)?.use { c ->
                if (c.moveToFirst()) { c.getString(0)?.let { name = it }; if (!c.isNull(1)) size = c.getLong(1) }
            }
        }
        return Triple(name, cr.getType(uri) ?: "application/octet-stream", size)
    }

    private suspend fun streamUpload(path: String, fields: Map<String, String>, uri: android.net.Uri) {
        val (name, mime, size) = picked(uri)
        val cr = getApplication<Application>().contentResolver
        api.uploadStream(path, fields, name, mime, size) { cr.openInputStream(uri) ?: throw java.io.FileNotFoundException(name) }
    }

    suspend fun driveList(folder: String, q: String = ""): DriveList? = ws?.let { w ->
        runCatching { api.get<DriveList>("/api/workspaces/$w/drive?folder=${enc(folder)}" + if (q.isNotBlank()) "&q=${enc(q)}" else "") }.onFailure(::fail).getOrNull()
    }

    /** Envoi de fichiers dans un dossier du drive, gardés [days] jours. Renvoie le nombre d'échecs. */
    suspend fun driveUpload(folder: String, days: Int, uris: List<android.net.Uri>): Int {
        val w = ws ?: return uris.size
        var failed = 0
        for (u in uris) runCatching { streamUpload("/api/workspaces/$w/drive/upload", mapOf("folder" to folder, "days" to days.toString()), u) }.onFailure { failed++; fail(it) }
        return failed
    }

    suspend fun driveCreateFolder(folder: String, name: String): Boolean = ws?.let { w ->
        runCatching { api.send("POST", "/api/workspaces/$w/drive/folders", buildJsonObject { put("folder", folder); put("name", name.trim()) }) }.onFailure(::fail).isSuccess
    } ?: false

    /** Renommer, déplacer (dossier de destination) ou prolonger ([days] à partir de maintenant). */
    suspend fun driveUpdate(id: String, name: String? = null, folder: String? = null, days: Int? = null): Boolean = ws?.let { w ->
        runCatching {
            api.send("PUT", "/api/workspaces/$w/drive/files/$id", buildJsonObject { name?.let { put("name", it.trim()) }; folder?.let { put("folder", it) }; days?.let { put("days", it) } })
        }.onFailure(::fail).isSuccess
    } ?: false

    suspend fun driveDelete(id: String): Boolean = ws?.let { w -> runCatching { api.send("DELETE", "/api/workspaces/$w/drive/files/$id") }.onFailure(::fail).isSuccess } ?: false

    suspend fun driveShare(id: String, on: Boolean): ShareLink? = ws?.let { w ->
        runCatching { if (on) api.post<ShareLink>("/api/workspaces/$w/drive/files/$id/share") else { api.send("DELETE", "/api/workspaces/$w/drive/files/$id/share"); null } }.onFailure(::fail).getOrNull()
    }

    /** Fichier du serveur enregistré dans le cache (pour l'ouvrir avec une autre application). */
    suspend fun cachedDownload(key: String, name: String, path: String): java.io.File? = runCatching {
        val f = java.io.File(getApplication<Application>().cacheDir, "attachments/$key/" + name.replace(Regex("[/\\\\:*?\"<>|]"), "_").ifBlank { "file" })
        if (!f.exists()) api.downloadTo(path, f)
        f
    }.onFailure(::fail).getOrNull()

    suspend fun driveFile(item: DriveItem): java.io.File? = ws?.let { w -> cachedDownload("drive-${item.id}", item.name, "/api/workspaces/$w/drive/files/${item.id}/download") }

    // Projets
    suspend fun projects(): List<ProjectSummary>? = ws?.let { w -> runCatching { api.get<List<ProjectSummary>>("/api/workspaces/$w/projects") }.onFailure(::fail).getOrNull() }

    suspend fun createProject(kind: String, name: String, template: String): String? = ws?.let { w ->
        runCatching { api.post<ProjectInfo>("/api/workspaces/$w/projects", buildJsonObject { put("kind", kind); put("name", name.trim()); put("template", template) }) }.onFailure(::fail).getOrNull()?.id
    }

    suspend fun project(id: String): ProjectFull? = runCatching { api.get<ProjectFull>("/api/projects/$id") }.onFailure(::fail).getOrNull()

    suspend fun updateProject(id: String, body: JsonObject): Boolean = runCatching { api.send("PUT", "/api/projects/$id", body) }.onFailure(::fail).isSuccess

    suspend fun deleteProject(id: String): Boolean = runCatching { api.send("DELETE", "/api/projects/$id") }.onFailure(::fail).isSuccess

    suspend fun createProjectFile(id: String, path: String, isDir: Boolean): Boolean =
        runCatching { api.send("POST", "/api/projects/$id/files", buildJsonObject { put("path", path.trim()); put("is_dir", isDir) }) }.onFailure(::fail).isSuccess

    suspend fun moveProjectFile(id: String, fid: String, path: String): Boolean =
        runCatching { api.send("PUT", "/api/projects/$id/files/$fid", buildJsonObject { put("path", path.trim()) }) }.onFailure(::fail).isSuccess

    suspend fun deleteProjectFile(id: String, fid: String): Boolean = runCatching { api.send("DELETE", "/api/projects/$id/files/$fid") }.onFailure(::fail).isSuccess

    suspend fun projectFileText(id: String, fid: String): String? = runCatching { api.raw("GET", "/api/projects/$id/files/$fid/raw") }.onFailure(::fail).getOrNull()

    suspend fun projectFileDownload(id: String, f: ProjectFile): java.io.File? =
        cachedDownload("project-$id-${f.id}-${f.updated_at.hashCode()}", f.path.substringAfterLast('/'), "/api/projects/$id/files/${f.id}/raw")

    suspend fun projectUpload(id: String, folder: String, uris: List<android.net.Uri>): Int {
        var failed = 0
        for (u in uris) runCatching { streamUpload("/api/projects/$id/upload", mapOf("folder" to folder), u) }.onFailure { failed++; fail(it) }
        return failed
    }

    suspend fun compileProject(id: String): CompileResult? = runCatching { api.post<CompileResult>("/api/projects/$id/compile") }.onFailure(::fail).getOrNull()

    /** PDF compilé : toujours téléchargé à nouveau (il change à chaque compilation). */
    suspend fun projectPdf(id: String, name: String): java.io.File? = runCatching {
        val f = java.io.File(getApplication<Application>().cacheDir, "attachments/pdf-$id/" + name.replace(Regex("[^\\p{L}\\p{N}_-]"), "_").ifBlank { "document" } + ".pdf")
        f.delete(); api.downloadTo("/api/projects/$id/output.pdf", f); f
    }.onFailure(::fail).getOrNull()

    suspend fun projectZip(id: String, name: String): java.io.File? = runCatching {
        val f = java.io.File(getApplication<Application>().cacheDir, "attachments/zip-$id/" + name.replace(Regex("[^\\p{L}\\p{N}_-]"), "_").ifBlank { "project" } + ".zip")
        f.delete(); api.downloadTo("/api/projects/$id/download", f); f
    }.onFailure(::fail).getOrNull()

    suspend fun projectPreview(id: String): String? = runCatching { api.post<PreviewBase>("/api/projects/$id/preview-token").base }.onFailure(::fail).getOrNull()

    // -----------------------------------------------------------------------
    // Activité, corbeille, graphe, Moodle (N5)
    // -----------------------------------------------------------------------

    suspend fun feed(user: String?, before: String?): Feed? = ws?.let { w ->
        val q = listOfNotNull(user?.let { "user=$it" }, before?.let { "before=${enc(it)}" }, "limit=50").joinToString("&")
        runCatching { api.get<Feed>("/api/workspaces/$w/activity?$q") }.onFailure(::fail).getOrNull()
    }

    suspend fun trash(): Trash? = ws?.let { w -> runCatching { api.get<Trash>("/api/workspaces/$w/trash") }.onFailure(::fail).getOrNull() }

    suspend fun restoreTrashed(kind: String, id: String): Boolean =
        runCatching { api.send("POST", "/api/trash/$kind/$id/restore") }.onFailure(::fail).isSuccess.also { if (it) { if (kind == "pages") loadPages() else { loadTasks(); loadOverview() } } }

    suspend fun deleteTrashed(kind: String, id: String): Boolean = runCatching { api.send("DELETE", "/api/trash/$kind/$id") }.onFailure(::fail).isSuccess

    suspend fun emptyTrash(): Boolean = ws?.let { w -> runCatching { api.send("DELETE", "/api/workspaces/$w/trash") }.onFailure(::fail).isSuccess } ?: false

    suspend fun graph(): Graph? = ws?.let { w -> runCatching { api.get<Graph>("/api/workspaces/$w/graph") }.onFailure(::fail).getOrNull() }

    // Moodle : l'erreur est renvoyée telle quelle (affichée dans l'écran).
    private fun moodle(path: String) = "/api/workspaces/${ws}/moodle$path"
    suspend fun moodleOverview(): MoodleOverview? = runCatching { api.get<MoodleOverview>(moodle("")) }.onFailure(::fail).getOrNull()
    suspend fun moodleSite(): Result<MoodleSite> = runCatching { api.get<MoodleSite>(moodle("/site")) }
    suspend fun moodleConnect(body: JsonObject): Result<Unit> = runCatching { api.send("POST", moodle("/connect"), body) }
    suspend fun moodleDisconnect(): Boolean = runCatching { api.send("DELETE", moodle("/connect")) }.onFailure(::fail).isSuccess
    suspend fun moodleNotify(on: Boolean): Boolean = runCatching { api.send("PUT", moodle("/preferences"), buildJsonObject { put("notify", on) }) }.onFailure(::fail).isSuccess
    suspend fun moodleSyncCalendar(): Boolean = runCatching { api.send("POST", moodle("/calendar/sync")) }.onFailure(::fail).isSuccess.also { if (it) loadCalendars() }
    /** Tous les cours, masqués compris (le filtre « masqués » les montre). */
    suspend fun moodleCourses(): Result<List<MoodleCourse>> = runCatching { api.get<List<MoodleCourse>>(moodle("/courses?all=true")) }
    suspend fun moodleSetCourseFlag(id: Long, flag: String, value: Boolean): Boolean =
        runCatching { api.send("PUT", moodle("/courses/$id/$flag"), buildJsonObject { put("value", value) }) }.onFailure(::fail).isSuccess

    // Dossiers, personnalisation et ordre des cours, tableau de bord : préférences du membre, partagées avec le web.
    var coursePrefs by mutableStateOf(CoursePrefs()); private set
    var boardPanels by mutableStateOf<List<BoardPanel>?>(null); private set
    suspend fun loadCoursePrefs() {
        val w = ws ?: return
        runCatching { api.get<JsonObject>("/api/workspaces/$w/prefs") }.onSuccess { p ->
            coursePrefs = p["moodleCourses"]?.let { runCatching { api.json.decodeFromJsonElement<CoursePrefs>(it) }.getOrNull() } ?: CoursePrefs()
            boardPanels = p["moodleBoard"]?.jsonObject?.get("panels")?.let { runCatching { api.json.decodeFromJsonElement<List<BoardPanel>>(it) }.getOrNull() }
        }
    }
    /** Enregistre aussitôt en local, puis sur le serveur ; en cas d'échec on revient à l'état d'avant. */
    suspend fun saveCoursePrefs(next: CoursePrefs): Boolean {
        val w = ws ?: return false
        val before = coursePrefs
        coursePrefs = next
        val ok = runCatching { api.send("PATCH", "/api/workspaces/$w/prefs", buildJsonObject { put("moodleCourses", api.json.encodeToJsonElement(next)) }) }.onFailure(::fail).isSuccess
        if (!ok) coursePrefs = before
        return ok
    }
    suspend fun moodleCourse(id: Long): Result<MoodleCourseContent> = runCatching { api.get<MoodleCourseContent>(moodle("/courses/$id")) }
    suspend fun moodleAssignment(course: Long, cmid: Long): Result<MoodleAssignment> = runCatching { api.get<MoodleAssignment>(moodle("/courses/$course/assign/$cmid")) }
    suspend fun moodlePage(source: String): Result<String> = runCatching { api.get<MoodleHtml>(moodle("/html?url=${enc(source)}")).html }
    /** Fichier d'un cours (servi par Looty) téléchargé dans le cache, pour l'ouvrir. */
    suspend fun moodleFile(path: String, name: String): Result<java.io.File> = runCatching {
        val f = java.io.File(getApplication<Application>().cacheDir, "attachments/moodle/" + name.replace(Regex("[/\\\\:*?\"<>|]"), "_").ifBlank { "file" })
        api.downloadTo(path, f); f
    }
    suspend fun moodleDeadlines(): Result<List<MoodleDeadline>> = runCatching { api.get<List<MoodleDeadline>>(moodle("/deadlines")) }
    suspend fun moodleNotifications(): Result<MoodleNotifs> = runCatching { api.get<MoodleNotifs>(moodle("/notifications")) }
    suspend fun moodleConversations(): Result<List<MoodleConversation>> = runCatching { api.get<List<MoodleConversation>>(moodle("/conversations")) }
    suspend fun moodleThread(id: Long): Result<MoodleThread> = runCatching { api.get<MoodleThread>(moodle("/conversations/$id")) }
    suspend fun moodleSend(id: Long, text: String): Boolean = runCatching { api.send("POST", moodle("/conversations/$id"), buildJsonObject { put("text", text) }) }.onFailure(::fail).isSuccess
    /** Lien Moodle ouvert déjà connecté (si la plateforme le permet). */
    suspend fun moodleOpen(url: String?): String? = runCatching { api.get<OpenUrl>(moodle("/open") + (url?.let { "?url=${enc(it)}" } ?: "")).url }.onFailure(::fail).getOrNull()

    // -----------------------------------------------------------------------
    // Réglages (N6)
    // -----------------------------------------------------------------------

    /** Rôle dans l'espace : owner, admin, member, viewer, guest. */
    val wsRole: String get() = access?.role ?: "member"

    private suspend fun refreshMe() {
        runCatching { api.get<Me>("/api/auth/me") }.getOrNull()?.user?.let { u -> (session as? Session.Ready)?.let { session = it.copy(me = u) } }
    }

    suspend fun saveProfile(name: String, avatarUrl: String?): Boolean =
        runCatching { api.send("PUT", "/api/user/profile", buildJsonObject { put("name", name.trim()); put("avatar_url", avatarUrl) }) }.onFailure(::fail).isSuccess.also { if (it) { refreshMe(); loadMembers() } }

    suspend fun savePreference(key: String, value: String): Boolean =
        runCatching { api.send("PUT", "/api/user/preferences", buildJsonObject { put(key, value) }) }.onFailure(::fail).isSuccess

    suspend fun notifPrefs(): NotifPrefs? = runCatching { api.get<NotifPrefs>("/api/user/notifications") }.onFailure(::fail).getOrNull()
    suspend fun saveNotifPrefs(body: JsonObject): NotifPrefs? = runCatching { api.json.decodeFromString<NotifPrefs>(api.raw("PUT", "/api/user/notifications", body.toString())) }.onFailure(::fail).getOrNull()
    suspend fun pushDevices(): List<PushDevice>? = runCatching { api.get<List<PushDevice>>("/api/user/push/subscriptions") }.onFailure(::fail).getOrNull()
    suspend fun removePushDevice(id: String): Boolean = runCatching { api.send("DELETE", "/api/user/push/subscriptions/$id") }.onFailure(::fail).isSuccess

    suspend fun apiTokens(): List<ApiToken>? = runCatching { api.get<List<ApiToken>>("/api/user/tokens") }.onFailure(::fail).getOrNull()
    suspend fun createApiToken(name: String): String? = runCatching { api.post<NewToken>("/api/user/tokens", buildJsonObject { put("name", name.trim()) }).token }.onFailure(::fail).getOrNull()
    suspend fun revokeApiToken(id: String): Boolean = runCatching { api.send("DELETE", "/api/user/tokens/$id") }.onFailure(::fail).isSuccess
    suspend fun identities(): List<Identity>? = runCatching { api.get<List<Identity>>("/api/user/identities") }.onFailure(::fail).getOrNull()
    suspend fun unlinkIdentity(provider: String): Boolean = runCatching { api.send("DELETE", "/api/user/identities/$provider") }.onFailure(::fail).isSuccess

    // Espace
    suspend fun renameWorkspace(name: String): Boolean = ws?.let { w ->
        runCatching { api.send("PUT", "/api/workspaces/$w", buildJsonObject { put("name", name.trim()) }) }.onFailure(::fail).isSuccess.also { if (it) start() }
    } ?: false
    suspend fun leaveWorkspace(): Boolean = ws?.let { w -> runCatching { api.send("POST", "/api/workspaces/$w/leave") }.onFailure(::fail).isSuccess } ?: false
    suspend fun deleteWorkspace(): Boolean = ws?.let { w -> runCatching { api.send("DELETE", "/api/workspaces/$w") }.onFailure(::fail).isSuccess } ?: false
    suspend fun setGroupTasks(on: Boolean): Boolean = ws?.let { w ->
        runCatching { api.send("PATCH", "/api/workspaces/$w/access/me", buildJsonObject { put("group_tasks", on) }) }.onFailure(::fail).isSuccess.also { if (it) loadAccess() }
    } ?: false
    suspend fun trashRetention(): Int? = trash()?.retention_days
    suspend fun setTrashRetention(days: Int): Boolean = ws?.let { w -> runCatching { api.send("PUT", "/api/workspaces/$w/trash/settings", buildJsonObject { put("retention_days", days) }) }.onFailure(::fail).isSuccess } ?: false
    suspend fun driveSettings(): WorkspaceDrive? = ws?.let { w -> runCatching { api.get<WorkspaceDrive>("/api/workspaces/$w/drive") }.getOrNull() }
    suspend fun setDriveSettings(days: Int, quotaMb: Int): Boolean = ws?.let { w ->
        runCatching { api.send("PUT", "/api/workspaces/$w/drive/settings", buildJsonObject { put("max_days", days); put("quota_mb", quotaMb) }) }.onFailure(::fail).isSuccess
    } ?: false
    suspend fun setMoodleUrl(url: String?): Boolean = ws?.let { w -> runCatching { api.send("PUT", "/api/workspaces/$w/moodle/settings", buildJsonObject { put("url", url) }) }.onFailure(::fail).isSuccess } ?: false

    // Membres
    suspend fun roles(): List<WsRole>? = ws?.let { w -> runCatching { api.get<List<WsRole>>("/api/workspaces/$w/access/roles") }.onFailure(::fail).getOrNull() }
    suspend fun setMemberRole(userId: String, roleId: String): Boolean = ws?.let { w ->
        runCatching { api.send("PUT", "/api/workspaces/$w/members/$userId", buildJsonObject { put("role_id", roleId) }) }.onFailure(::fail).isSuccess.also { if (it) loadMembers() }
    } ?: false
    suspend fun removeMember(userId: String): Boolean = ws?.let { w ->
        runCatching { api.send("DELETE", "/api/workspaces/$w/members/$userId") }.onFailure(::fail).isSuccess.also { if (it) loadMembers() }
    } ?: false
    suspend fun searchUsers(q: String): List<UserHit> = ws?.let { w -> runCatching { api.get<List<UserHit>>("/api/users/search?q=${enc(q)}&exclude_ws=$w") }.getOrNull() }.orEmpty()
    suspend fun addMember(userId: String?, email: String?, roleId: String?): Result<AddResult> = runCatching {
        val w = ws ?: throw ApiException(0, "no_workspace")
        api.post<AddResult>("/api/workspaces/$w/access/members/add", buildJsonObject { userId?.let { put("user_id", it) }; email?.let { put("email", it.trim()) }; roleId?.let { put("role_id", it) } })
    }.also { if (it.isSuccess) loadMembers() }
    suspend fun invites(): List<Invite>? = ws?.let { w -> runCatching { api.get<List<Invite>>("/api/workspaces/$w/invites") }.getOrNull() }
    suspend fun revokeInvite(id: String): Boolean = ws?.let { w -> runCatching { api.send("DELETE", "/api/workspaces/$w/invites/$id") }.onFailure(::fail).isSuccess } ?: false

    // Synchronisation CalDAV des tâches
    suspend fun calDavAccounts(): List<CalDavAccount>? = runCatching { api.get<List<CalDavAccount>>("/api/user/caldav") }.onFailure(::fail).getOrNull()
    suspend fun discoverCalDav(server: String, user: String, password: String): Result<List<DiscoveredCalendar>> = runCatching {
        api.json.decodeFromString<List<DiscoveredCalendar>>(api.raw("POST", "/api/user/caldav/discover", buildJsonObject { put("server_url", server.trim()); put("username", user.trim()); put("password", password) }.toString()))
    }
    suspend fun createCalDav(body: JsonObject): Result<Unit> = runCatching { api.send("POST", "/api/user/caldav", body) }
    suspend fun updateCalDav(id: String, body: JsonObject): Boolean = runCatching { api.send("PUT", "/api/user/caldav/$id", body) }.onFailure(::fail).isSuccess
    suspend fun deleteCalDav(id: String): Boolean = runCatching { api.send("DELETE", "/api/user/caldav/$id") }.onFailure(::fail).isSuccess
    suspend fun syncCalDav(id: String): SyncReport? = runCatching { api.post<SyncResult>("/api/user/caldav/$id/sync").report }.onFailure(::fail).getOrNull()

    // -----------------------------------------------------------------------
    // Calendrier (G3)
    // -----------------------------------------------------------------------

    var calSources by mutableStateOf<List<CalendarSource>>(emptyList()); private set
    var calRange by mutableStateOf<Pair<LocalDate, LocalDate>?>(null); private set
    var calEvents by mutableStateOf(Loadable<CalEvents>()); private set
    /** Calendriers masqués (propres à l'appareil) ; « workspace » = calendrier de l'espace. */
    var hiddenCalendars by mutableStateOf(prefs.getStringSet("hiddenCalendars", emptySet()).orEmpty()); private set

    fun loadCalendars() {
        val w = ws ?: return
        viewModelScope.launch {
            runCatching {
                // Calendriers Looty de l'espace (par défaut, thématiques, tableaux) puis abonnements.
                val looty = runCatching { api.get<List<LootyCalendar>>("/api/workspaces/$w/planning/calendars") }.getOrDefault(emptyList())
                    .map { CalendarSource(it.id, if (it.is_default) "" else it.name, it.color, "looty", "workspace") }
                val all = looty + api.get<List<CalendarSource>>("/api/workspaces/$w/calendars")
                calSources = all
                markOnline()
                withContext(Dispatchers.IO) { cache.put("${w}/calsources", api.json.encodeToString(all)) }
            }.onFailure {
                if (it.isOffline()) markOffline()
                if (calSources.isEmpty()) withContext(Dispatchers.IO) { cache.get("${w}/calsources") }?.let { raw -> runCatching { calSources = api.json.decodeFromString<List<CalendarSource>>(raw) } }
            }
        }
    }

    fun loadEvents(from: LocalDate, to: LocalDate) {
        val w = ws ?: return
        calRange = from to to
        val zone = ZoneId.systemDefault()
        val f = from.atStartOfDay(zone).toInstant()
        val t = to.atStartOfDay(zone).toInstant()
        load({ calEvents }, { calEvents = it }, "${w}/events-${from}-${to}") { api.get<CalEvents>("/api/workspaces/$w/calendars/events?from=$f&to=$t") }
    }

    private fun reloadEvents() { calRange?.let { (f, t) -> loadEvents(f, t) }; loadUpcoming() }

    fun toggleCalendar(id: String) {
        hiddenCalendars = if (id in hiddenCalendars) hiddenCalendars - id else hiddenCalendars + id
        prefs.edit().putStringSet("hiddenCalendars", hiddenCalendars).apply()
    }

    suspend fun eventDetail(id: String): EventDetail? = ws?.let { w ->
        runCatching { api.get<EventDetail>("/api/workspaces/$w/events/$id").also { d -> withContext(Dispatchers.IO) { cache.put("event-$id", api.json.encodeToString(d)) } } }.getOrNull()
            ?: withContext(Dispatchers.IO) { cache.get("event-$id") }?.let { raw -> runCatching { api.json.decodeFromString<EventDetail>(raw) }.getOrNull() }
    }

    /** Création ou modification d'un événement de l'espace (corps comme l'interface web). */
    suspend fun saveEvent(id: String?, body: JsonObject): Result<Unit> = runCatching {
        val w = ws ?: throw ApiException(0, "no_workspace")
        if (id == null) write("POST", "/api/workspaces/$w/events", body, body["title"]?.toString()?.trim('"') ?: "event") else write("PUT", "/api/workspaces/$w/events/$id", body, body["title"]?.toString()?.trim('"') ?: "event")
        reloadEvents()
    }

    /** Supprime l'événement, ou une seule occurrence d'une série. */
    fun deleteEvent(e: CalEvent, onlyThis: Boolean) {
        val w = ws ?: return
        val id = e.event_id ?: return
        calEvents = calEvents.copy(data = calEvents.data?.let { d -> d.copy(events = d.events.filter { if (onlyThis) it.id != e.id else it.event_id != id }) })
        viewModelScope.launch {
            val q = if (onlyThis && e.recurring) "?occurrence=${java.net.URLEncoder.encode(e.starts_at, "UTF-8")}" else ""
            runCatching { write("DELETE", "/api/workspaces/$w/events/$id$q", null, e.summary) }.onFailure { lastError = (it as? ApiException)?.code ?: "offline" }
            reloadEvents()
        }
    }

    // -----------------------------------------------------------------------
    // Mail (G3, N2). La boîte de réception passe par la copie du serveur (rapide,
    // hors ligne) ; les autres dossiers, la recherche et les suivis sont lus en direct.
    // -----------------------------------------------------------------------

    var mailAccounts by mutableStateOf(Loadable<List<MailAccount>>()); private set
    var mailAccount by mutableStateOf(prefs.getString("mailAccount", null)); private set
    var mailUnreadOnly by mutableStateOf(false); private set
    var mailFlaggedOnly by mutableStateOf(false); private set
    var mailFolder by mutableStateOf("INBOX"); private set
    var mailSearch by mutableStateOf(""); private set
    var mailFolders by mutableStateOf<MailFolders?>(null); private set
    var mailList by mutableStateOf(Loadable<List<MailSummary>>()); private set
    var mailHasMore by mutableStateOf(false); private set
    var mailLoadingMore by mutableStateOf(false); private set
    /** Dernier message ouvert (réponse, transfert). */
    var openMail by mutableStateOf<MailMessage?>(null)

    val currentMailAccount: MailAccount? get() = mailAccounts.data?.let { l -> l.firstOrNull { it.id == mailAccount } ?: l.firstOrNull() }
    val mailUnread: Int get() = mailAccounts.data?.sumOf { it.unread } ?: 0
    val currentFolder: MailFolder? get() = mailFolders?.folders?.firstOrNull { it.name == mailFolder }
    private val mailLive: Boolean get() = !mailFolder.equals("INBOX", true) || mailSearch.isNotBlank() || mailFlaggedOnly

    fun loadMailAccounts() {
        load({ mailAccounts }, { mailAccounts = it }, "mail-accounts") { api.get<List<MailAccount>>("/api/mail/accounts") }
    }

    fun selectMailAccount(id: String) {
        mailAccount = id
        prefs.edit().putString("mailAccount", id).apply()
        mailFolder = "INBOX"; mailSearch = ""; mailFolders = null
        loadMail(); loadMailFolders()
    }

    fun setMailFilter(unread: Boolean, flagged: Boolean = false) { mailUnreadOnly = unread; mailFlaggedOnly = flagged; loadMail() }
    fun selectMailFolder(name: String) { mailFolder = name; mailSearch = ""; mailFlaggedOnly = false; mailUnreadOnly = false; loadMail() }
    fun searchMail(q: String) { mailSearch = q.trim(); loadMail() }

    private fun folderQuery(acc: String, before: Long?): String {
        val p = mutableListOf("name=" + java.net.URLEncoder.encode(mailFolder, "UTF-8"))
        if (before != null) p += "before=$before"
        if (mailSearch.isNotBlank()) p += "q=" + java.net.URLEncoder.encode(mailSearch, "UTF-8")
        if (mailUnreadOnly) p += "unread=true"
        if (mailFlaggedOnly) p += "flagged=true"
        return "/api/mail/accounts/$acc/folder?" + p.joinToString("&")
    }

    fun loadMail() {
        val acc = currentMailAccount?.id ?: return
        val live = mailLive
        load({ mailList }, { mailList = it }, if (live) "mailf-$acc-$mailFolder-$mailUnreadOnly-$mailFlaggedOnly-$mailSearch" else "mail-${acc}-${mailUnreadOnly}") {
            val page = if (live) api.get<MailPage>(folderQuery(acc, null)) else api.get<MailPage>("/api/mail/accounts/$acc/messages${if (mailUnreadOnly) "?unread=true" else ""}")
            mailHasMore = page.has_more
            if (live) page.messages else page.messages.map { it.copy(flagged = it.uid in starredInbox) }
        }
        // La copie de la boîte de réception ne connaît pas les suivis : ils sont demandés à part.
        if (!live) viewModelScope.launch {
            val starred = runCatching { api.get<MailPage>("/api/mail/accounts/$acc/folder?name=INBOX&flagged=true") }.getOrNull()?.messages?.map { it.uid }?.toSet() ?: return@launch
            starredInbox = starred
        }
    }

    /** Messages suivis de la boîte de réception (la copie du serveur ne les connaît pas). */
    private var starredInbox: Set<Long> = emptySet()
        set(v) { field = v; if (!mailLive) mailList = mailList.copy(data = mailList.data?.map { it.copy(flagged = it.uid in v) }) }

    fun loadMoreMail() {
        val acc = currentMailAccount?.id ?: return
        val last = mailList.data?.lastOrNull()?.uid ?: return
        if (mailLoadingMore) return
        mailLoadingMore = true
        viewModelScope.launch {
            runCatching {
                if (mailLive) api.get<MailPage>(folderQuery(acc, last))
                else api.get<MailPage>("/api/mail/accounts/$acc/messages?before=$last${if (mailUnreadOnly) "&unread=true" else ""}")
            }.onSuccess { p ->
                mailList = mailList.copy(data = mailList.data.orEmpty() + if (mailLive) p.messages else p.messages.map { it.copy(flagged = it.uid in starredInbox) })
                mailHasMore = p.has_more
            }
            mailLoadingMore = false
        }
    }

    fun loadMailFolders() {
        val acc = currentMailAccount?.id ?: return
        fetchCached("mail-folders-$acc", { f: MailFolders -> if (currentMailAccount?.id == acc) mailFolders = f }) { api.get<MailFolders>("/api/mail/accounts/$acc/folders") }
    }

    /** Relève la boîte maintenant (serveur IMAP), puis recharge. */
    fun refreshMail() {
        val acc = currentMailAccount?.id ?: return
        mailList = mailList.copy(loading = true)
        viewModelScope.launch {
            runCatching { api.send("POST", "/api/mail/accounts/$acc/refresh") }
            loadMailAccounts(); loadMail(); loadMailFolders()
        }
    }

    private fun isInbox(folder: String) = folder.equals("INBOX", true)

    private fun localSeen(uid: Long, seen: Boolean, folder: String = mailFolder) {
        if (folder != mailFolder) return
        val was = mailList.data?.firstOrNull { it.uid == uid }?.seen ?: return
        mailList = mailList.copy(data = mailList.data?.map { if (it.uid == uid) it.copy(seen = seen) else it })
        if (was != seen && isInbox(folder)) mailAccounts = mailAccounts.copy(data = mailAccounts.data?.map { if (it.id == currentMailAccount?.id) it.copy(unread = (it.unread + if (seen) -1 else 1).coerceAtLeast(0)) else it })
        if (was != seen) mailFolders = mailFolders?.let { f -> f.copy(folders = f.folders.map { if (it.name == folder) it.copy(unread = (it.unread + if (seen) -1 else 1).coerceAtLeast(0)) else it }) }
    }

    fun setSeen(uid: Long, seen: Boolean, folder: String = mailFolder) {
        val acc = currentMailAccount?.id ?: return
        localSeen(uid, seen, folder)
        viewModelScope.launch {
            runCatching {
                if (isInbox(folder)) write("PUT", "/api/mail/accounts/$acc/messages/$uid/seen", buildJsonObject { put("seen", seen) }, "mail")
                else api.send("POST", "/api/mail/accounts/$acc/folder/flags", buildJsonObject { put("name", folder); putJsonArray("uids") { add(uid) }; put("seen", seen) })
            }.onFailure { localSeen(uid, !seen, folder); fail(it) }
        }
    }

    fun setFlagged(uid: Long, flagged: Boolean, folder: String = mailFolder) {
        val acc = currentMailAccount?.id ?: return
        fun local(on: Boolean) {
            if (isInbox(folder)) starredInbox = if (on) starredInbox + uid else starredInbox - uid
            if (folder == mailFolder) mailList = mailList.copy(data = mailList.data?.map { if (it.uid == uid) it.copy(flagged = on) else it })
            openMail = openMail?.let { if (it.uid == uid && it.folder == folder) it.copy(flagged = on) else it }
        }
        local(flagged)
        viewModelScope.launch {
            runCatching { api.send("POST", "/api/mail/accounts/$acc/folder/flags", buildJsonObject { put("name", folder); putJsonArray("uids") { add(uid) }; put("flagged", flagged) }) }
                .onFailure { local(!flagged); fail(it) }
        }
    }

    private fun removeLocal(uid: Long, folder: String) {
        if (folder == mailFolder) mailList = mailList.copy(data = mailList.data?.filter { it.uid != uid })
    }

    /** Déplace un message ; la liste et les compteurs sont rechargés ensuite. */
    fun moveMail(uid: Long, from: String, to: String) {
        val acc = currentMailAccount?.id ?: return
        removeLocal(uid, from)
        viewModelScope.launch {
            runCatching { api.send("POST", "/api/mail/accounts/$acc/folder/move", buildJsonObject { put("name", from); putJsonArray("uids") { add(uid) }; put("to", to) }) }.onFailure(::fail)
            loadMail(); loadMailFolders(); loadMailAccounts()
        }
    }

    /** Vers la corbeille (ou définitivement depuis la corbeille). Renvoie true si c'est définitif. */
    suspend fun deleteMail(uid: Long, folder: String): Boolean? {
        val acc = currentMailAccount?.id ?: return null
        removeLocal(uid, folder)
        val r = runCatching { api.post<DeleteResult>("/api/mail/accounts/$acc/folder/delete", buildJsonObject { put("name", folder); putJsonArray("uids") { add(uid) } }) }.onFailure(::fail).getOrNull()
        loadMail(); loadMailFolders(); loadMailAccounts()
        return r?.permanent
    }

    fun readAllMail() {
        val acc = currentMailAccount?.id ?: return
        viewModelScope.launch { runCatching { api.send("POST", "/api/mail/accounts/$acc/read-all") }; loadMailAccounts(); loadMail(); loadMailFolders() }
    }

    /** Lecture d'un message (marqué lu par le serveur) ; hors ligne, la copie de la boîte de réception. */
    suspend fun readMail(uid: Long, folder: String = mailFolder): MailMessage? {
        val acc = currentMailAccount?.id ?: return null
        localSeen(uid, true, folder)
        val m = runCatching { api.get<MailMessage>("/api/mail/accounts/$acc/folder/message?name=${java.net.URLEncoder.encode(folder, "UTF-8")}&uid=$uid") }.getOrNull()
            ?: if (isInbox(folder)) runCatching { api.get<MailMessage>("/api/mail/accounts/$acc/messages/$uid") }.getOrNull() else null
        if (m != null) openMail = m.copy(folder = folder)
        return openMail?.takeIf { m != null }
    }

    /** Pièce jointe d'un message, enregistrée dans le cache de l'application pour l'ouvrir. */
    suspend fun mailAttachmentFile(m: MailMessage, a: MailAttachment): java.io.File? = runCatching {
        val acc = currentMailAccount?.id ?: throw ApiException(0, "no_account")
        val dir = java.io.File(getApplication<Application>().cacheDir, "attachments/mail-$acc-${m.folder.hashCode()}-${m.uid}-${a.index}")
        val f = java.io.File(dir, a.name.replace(Regex("[/\\\\:*?\"<>|]"), "_").ifBlank { "attachment" })
        if (!f.exists()) {
            val bytes = api.bytes("/api/mail/accounts/$acc/folder/attachment?name=${java.net.URLEncoder.encode(m.folder, "UTF-8")}&uid=${m.uid}&index=${a.index}")
            withContext(Dispatchers.IO) { dir.mkdirs(); f.writeBytes(bytes) }
        }
        f
    }.onFailure(::fail).getOrNull()

    suspend fun mailContacts(q: String): List<Contact> =
        runCatching { api.get<List<Contact>>("/api/mail/contacts?q=" + java.net.URLEncoder.encode(q, "UTF-8")) }.getOrDefault(emptyList())

    /** Fichier envoyé sur Looty (pièce jointe d'un message). */
    suspend fun uploadFile(f: SharedFile): Result<Uploaded> = runCatching { api.json.decodeFromString<Uploaded>(api.upload("/api/upload", f.name, f.mime, f.bytes)) }

    /** Envoi ; renvoie un avertissement éventuel (copie non enregistrée dans « Envoyés »). */
    suspend fun sendMail(accountId: String, body: JsonObject): Result<SendResult> =
        runCatching { api.post<SendResult>("/api/mail/accounts/$accountId/send", body) }.also { if (it.isSuccess) { loadMailFolders(); if (mailLive) loadMail() } }

    // Dossiers
    suspend fun createMailFolder(label: String, parent: String?): String? {
        val acc = currentMailAccount?.id ?: return null
        return runCatching { api.post<FolderCreated>("/api/mail/accounts/$acc/folders", buildJsonObject { put("label", label.trim()); parent?.let { put("parent", it) } }) }
            .onFailure(::fail).getOrNull()?.name.also { loadMailFolders() }
    }

    suspend fun renameMailFolder(name: String, label: String) {
        val acc = currentMailAccount?.id ?: return
        val r = runCatching { api.post<FolderCreated>("/api/mail/accounts/$acc/folders/rename", buildJsonObject { put("name", name); put("label", label.trim()) }) }.onFailure(::fail).getOrNull()
        if (r != null && mailFolder == name) selectMailFolder(r.name)
        loadMailFolders()
    }

    suspend fun deleteMailFolder(name: String) {
        val acc = currentMailAccount?.id ?: return
        val ok = runCatching { api.send("POST", "/api/mail/accounts/$acc/folders/delete", buildJsonObject { put("name", name) }) }.onFailure(::fail).isSuccess
        if (ok && (mailFolder == name || mailFolder.startsWith(name + (mailFolders?.delimiter ?: "/")))) selectMailFolder("INBOX")
        loadMailFolders()
    }

    suspend fun emptyMailFolder(name: String) {
        val acc = currentMailAccount?.id ?: return
        runCatching { api.send("POST", "/api/mail/accounts/$acc/folders/empty", buildJsonObject { put("name", name) }) }.onFailure(::fail)
        loadMailFolders(); if (mailFolder == name) loadMail()
    }

    // Comptes : l'erreur est renvoyée (affichée dans le formulaire).
    suspend fun createMailAccount(body: JsonObject): Result<MailAccount> =
        runCatching { api.post<MailAccount>("/api/mail/accounts", body) }.onSuccess { selectMailAccount(it.id); loadMailAccounts() }

    suspend fun updateMailAccount(id: String, body: JsonObject): Result<Unit> =
        runCatching { api.send("PUT", "/api/mail/accounts/$id", body) }.onSuccess { loadMailAccounts() }

    suspend fun deleteMailAccount(id: String): Boolean =
        runCatching { api.send("DELETE", "/api/mail/accounts/$id") }.onFailure(::fail).isSuccess.also {
            if (it) { if (mailAccount == id) { mailAccount = null; prefs.edit().remove("mailAccount").apply(); mailList = Loadable(); mailFolders = null }; loadMailAccounts() }
        }

    fun logout(onDone: () -> Unit) {
        viewModelScope.launch {
            // Cet appareil ne doit plus recevoir les notifications de ce compte.
            PushStore.subscriptionId(getApplication())?.let { sub -> runCatching { api.send("DELETE", "/api/user/push/subscriptions/$sub") }; PushStore.setSubscriptionId(getApplication(), null) }
            Push.disable(getApplication())
            runCatching { api.send("POST", "/api/auth/logout") }
            android.webkit.CookieManager.getInstance().removeAllCookies(null)
            withContext(Dispatchers.IO) { cache.clear() }
            prefs.edit().remove("assetVersion").apply()
            outbox.clear(); pendingChanges = 0; offline = false
            session = Session.NeedLogin
            onDone()
        }
    }
}
