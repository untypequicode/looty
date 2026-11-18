package io.github.untypequicode.looty.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.Session
import io.github.untypequicode.looty.api.*
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

private const val AI = "/api/ai"

/** Erreur du fournisseur (« provider_http_503: … » dans le flux) → phrase courte ; sinon code commun. */
private fun aiError(ctx: android.content.Context, raw: String): String {
    val m = Regex("^provider_http_(\\d{3})").find(raw)
    if (m != null) {
        val status = m.groupValues[1].toInt()
        val res = when {
            status == 429 || Regex("quota|RESOURCE_EXHAUSTED|rate.?limit", RegexOption.IGNORE_CASE).containsMatchIn(raw) -> R.string.ai_err_quota
            status == 401 || status == 403 -> R.string.ai_err_auth
            status == 404 -> R.string.ai_err_model
            status in 502..504 || Regex("overloaded|UNAVAILABLE", RegexOption.IGNORE_CASE).containsMatchIn(raw) -> R.string.ai_err_busy
            else -> R.string.ai_err_server
        }
        return ctx.getString(res)
    }
    if (raw.startsWith("provider_unreachable")) return ctx.getString(R.string.ai_err_unreachable)
    return errorText(ctx, raw)
}

private fun untilLabel(iso: String): String {
    val t = runCatching { Instant.parse(iso) }.getOrNull() ?: return ""
    return t.atZone(ZoneId.systemDefault()).format(DateTimeFormatter.ofLocalizedDateTime(FormatStyle.SHORT))
}

/** Liste des conversations avec l'assistant (la connexion au fournisseur se règle sur le web). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AiListScreen(state: AppState, onBack: () -> Unit, onOpen: (String) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var convs by remember { mutableStateOf<List<AiConversation>?>(null) }
    var conns by remember { mutableStateOf<List<AiConnection>?>(null) }
    var loading by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf(false) }
    var pick by remember { mutableStateOf(false) }
    var confirm by remember { mutableStateOf<AiConversation?>(null) }
    val snackbar = remember { SnackbarHostState() }
    fun load() {
        scope.launch {
            loading = true
            runCatching {
                conns = state.api.get<List<AiConnection>>("$AI/connections")
                convs = state.api.get<List<AiConversation>>("$AI/conversations")
            }.onFailure { if (it !is CancellationException) failed = true else throw it }.onSuccess { failed = false }
            loading = false
        }
    }
    fun create(c: AiConnection) {
        scope.launch {
            runCatching {
                state.api.post<AiConversation>("$AI/conversations", buildJsonObject { put("connection_id", c.id); put("context_kind", "none") })
            }.onSuccess { onOpen(it.id) }.onFailure { if (it is CancellationException) throw it; snackbar.showSnackbar(errorText(ctx, it)) }
        }
    }
    LaunchedEffect(Unit) { load() }
    val usable = conns.orEmpty()
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.ai_title)) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
        floatingActionButton = {
            if (usable.isNotEmpty()) ExtendedFloatingActionButton(
                onClick = { if (usable.size == 1) create(usable[0]) else pick = true },
                icon = { Icon(Icons.Filled.Add, null) }, text = { Text(stringResource(R.string.ai_new_conversation)) },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        PullToRefreshBox(isRefreshing = loading && convs != null, onRefresh = { load() }, modifier = Modifier.padding(padding).fillMaxSize()) {
            val list = convs
            when {
                list == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { if (failed) Text(stringResource(R.string.error_generic)) else LoadingIndicatorCompat() }
                usable.isEmpty() -> EmptyState(Icons.Outlined.AutoAwesome, stringResource(R.string.ai_no_connection))
                list.isEmpty() -> EmptyState(Icons.Outlined.ChatBubbleOutline, stringResource(R.string.ai_no_conversations))
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 88.dp)) {
                    items(list, key = { it.id }) { c ->
                        ListItem(
                            headlineContent = { Text(c.title.ifBlank { stringResource(R.string.ai_new_conversation) }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                            supportingContent = c.updated_at?.let { { Text(relativeTime(it, ctx)) } },
                            leadingContent = { Icon(Icons.Outlined.ChatBubbleOutline, null) },
                            trailingContent = { IconButton(onClick = { confirm = c }) { Icon(Icons.Outlined.Delete, stringResource(R.string.delete)) } },
                            modifier = Modifier.clickable { onOpen(c.id) },
                        )
                    }
                }
            }
        }
    }
    if (pick) AlertDialog(
        onDismissRequest = { pick = false },
        title = { Text(stringResource(R.string.ai_pick_connection)) },
        text = {
            Column {
                usable.forEach { c ->
                    ListItem(headlineContent = { Text(c.label.ifBlank { c.provider }) }, supportingContent = { Text(c.model) }, colors = sheetItemColors(),
                        modifier = Modifier.clickable { pick = false; create(c) })
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = { pick = false }) { Text(stringResource(R.string.cancel)) } },
    )
    confirm?.let { c ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text(stringResource(R.string.ai_delete_conversation)) },
            text = { Text(c.title) },
            confirmButton = {
                TextButton(onClick = {
                    confirm = null
                    scope.launch {
                        runCatching { state.api.send("DELETE", "$AI/conversations/${c.id}") }
                            .onSuccess { convs = convs?.filter { it.id != c.id } }
                            .onFailure { if (it is CancellationException) throw it; snackbar.showSnackbar(errorText(ctx, it)) }
                    }
                }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

private sealed interface ChatItem { val key: String }
private data class MsgItem(override val key: String, val user: Boolean, val text: String, val error: Boolean = false) : ChatItem
private data class ToolItem(override val key: String, val name: String, val ok: Boolean) : ChatItem
private data class SuggItem(override val key: String, val s: AiSuggestion) : ChatItem
private data class NeedItem(override val key: String, val need: AiGrantNeed) : ChatItem

private fun needKey(n: AiGrantNeed) = when (n.kind) {
    "page" -> "p:${n.pageId}"
    "study_update" -> "u:${n.setId}"
    else -> "c:${n.workspaceId ?: "personal"}"
}

/** Une carte « Autoriser » ancienne est périmée dès qu'une autorisation active la couvre. */
private fun covered(n: AiGrantNeed, grants: List<AiGrant>, study: List<AiStudyGrant>) = when (n.kind) {
    "page" -> grants.any { it.page_id == n.pageId }
    "study_update" -> study.any { it.kind == "update" && it.set_id == n.setId }
    else -> study.any { it.kind == "create" && (if (n.personal) it.personal else it.workspace_id == n.workspaceId) }
}

private val DURATIONS = listOf("15 min" to 15, "1 h" to 60, "4 h" to 240, "24 h" to 1440)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AiChatScreen(state: AppState, id: String, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }
    val items = remember { mutableStateListOf<ChatItem>() }
    var title by remember { mutableStateOf("") }
    var loaded by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf(false) }
    var input by remember { mutableStateOf("") }
    var streaming by remember { mutableStateOf(false) }
    var job by remember { mutableStateOf<Job?>(null) }
    var grants by remember { mutableStateOf<List<AiGrant>>(emptyList()) }
    var study by remember { mutableStateOf<List<AiStudyGrant>>(emptyList()) }
    var needs by remember { mutableStateOf<List<AiGrantNeed>>(emptyList()) }
    var sheet by remember { mutableStateOf(false) }
    val ws = (state.session as? Session.Ready)?.workspaceId
    val listState = rememberLazyListState()
    var seq by remember { mutableIntStateOf(0) }
    fun err(e: Throwable) { scope.launch { snackbar.showSnackbar(errorText(ctx, e)) } }

    fun refreshGrants() {
        scope.launch {
            runCatching {
                grants = state.api.get<List<AiGrant>>("$AI/conversations/$id/grants")
                study = state.api.get<List<AiStudyGrant>>("$AI/conversations/$id/study-grants")
            }.onFailure { if (it is CancellationException) throw it }
        }
    }

    LaunchedEffect(id) {
        runCatching { state.api.get<AiConversationDetail>("$AI/conversations/$id") }
            .onSuccess { d ->
                title = d.conversation.title
                grants = d.grants; study = d.study_grants
                val latestNeed = linkedMapOf<String, AiGrantNeed>()
                val out = mutableListOf<ChatItem>()
                for (m in d.messages) {
                    if (m.role == "tool") {
                        val g = (m.data as? JsonObject)?.get("grant_needed") as? JsonObject
                        parseGrantNeed(g ?: continue)?.let { latestNeed[needKey(it)] = it }
                    } else if (m.content.isNotBlank()) out.add(MsgItem(m.id, m.role == "user", m.content))
                }
                latestNeed.values.filter { !covered(it, d.grants, d.study_grants) }.forEach { out.add(NeedItem("need-" + needKey(it), it)) }
                d.suggestions.forEach { out.add(SuggItem("sug-" + it.id, it)) }
                items.clear(); items.addAll(out)
                loaded = true
            }
            .onFailure { if (it is CancellationException) throw it; failed = true }
    }
    DisposableEffect(Unit) { onDispose { job?.cancel() } }
    val lastLen = (items.lastOrNull() as? MsgItem)?.text?.length ?: 0
    LaunchedEffect(items.size, lastLen) { if (items.isNotEmpty()) listState.scrollToItem(items.lastIndex) }

    fun apply(ev: AiEvent) {
        when (ev) {
            is AiEvent.Text -> {
                val last = items.lastOrNull()
                if (last is MsgItem && !last.user && !last.error) items[items.lastIndex] = last.copy(text = last.text + ev.delta)
                else items.add(MsgItem("live-${seq++}", false, ev.delta))
            }
            is AiEvent.Tool -> items.add(ToolItem("tool-${seq++}", ev.name, ev.ok))
            is AiEvent.Suggestion -> items.add(SuggItem("sug-" + ev.suggestion.id, ev.suggestion))
            is AiEvent.PageChanged -> state.loadPages()
            is AiEvent.GrantNeeded -> if (!covered(ev.need, grants, study)) {
                items.removeAll { it is NeedItem && it.key == "need-" + needKey(ev.need) }
                items.add(NeedItem("need-" + needKey(ev.need), ev.need))
            }
            is AiEvent.Title -> title = ev.title
            is AiEvent.Error -> items.add(MsgItem("err-${seq++}", false, aiError(ctx, ev.message), error = true))
            AiEvent.Done -> {}
        }
    }

    fun send() {
        val text = input.trim()
        if (text.isEmpty() || streaming) return
        input = ""
        items.add(MsgItem("u-${seq++}", true, text))
        streaming = true
        job = scope.launch {
            try {
                state.api.stream("$AI/conversations/$id/messages", buildJsonObject { put("content", text); if (ws != null) put("workspace_id", ws) }) { data ->
                    val ev = parseAiEvent(state.api.json, data) ?: return@stream
                    withContext(Dispatchers.Main) { apply(ev) }
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                val code = (e as? ApiException)?.code ?: "offline"
                items.add(MsgItem("err-${seq++}", false, aiError(ctx, code), error = true))
            } finally {
                streaming = false
            }
        }
    }

    fun grantFor(n: AiGrantNeed) {
        scope.launch {
            val body = when (n.kind) {
                "page" -> buildJsonObject { put("page_id", n.pageId ?: return@launch); put("mode", n.operation ?: "append"); put("expires_in_minutes", 60) }
                "study_update" -> buildJsonObject { put("set_id", n.setId ?: return@launch); put("expires_in_minutes", 60) }
                else -> buildJsonObject { if (n.personal) put("personal", true) else put("workspace_id", n.workspaceId ?: return@launch); put("expires_in_minutes", 60) }
            }
            val path = if (n.kind == "page") "grants" else "study-grants"
            runCatching { state.api.send("POST", "$AI/conversations/$id/$path", body) }
                .onSuccess { items.removeAll { it is NeedItem && it.key == "need-" + needKey(n) }; refreshGrants() }
                .onFailure { if (it is CancellationException) throw it; err(it) }
        }
    }

    fun answer(s: AiSuggestion, accept: Boolean) {
        scope.launch {
            runCatching { state.api.send("POST", "$AI/suggestions/${s.id}/${if (accept) "accept" else "reject"}") }
                .onSuccess {
                    items.removeAll { it is SuggItem && it.s.id == s.id }
                    if (accept) state.loadPages()
                    snackbar.showSnackbar(ctx.getString(if (accept) R.string.ai_suggestion_accepted else R.string.ai_suggestion_rejected))
                }
                .onFailure { if (it is CancellationException) throw it; err(it) }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title.ifBlank { stringResource(R.string.ai_new_conversation) }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    IconButton(onClick = { sheet = true }) {
                        BadgedBox(badge = { if (grants.size + study.size > 0) Badge { Text((grants.size + study.size).toString()) } }) { Icon(Icons.Outlined.Shield, stringResource(R.string.ai_grants)) }
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
        contentWindowInsets = WindowInsets.safeDrawing,
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize().imePadding()) {
            Box(Modifier.weight(1f).fillMaxWidth()) {
                when {
                    failed -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text(stringResource(R.string.error_generic)) }
                    !loaded -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                    else -> LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = PaddingValues(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        items(items.toList(), key = { it.key }) { item ->
                            when (item) {
                                is MsgItem -> Bubble(item)
                                is ToolItem -> AssistChip(onClick = {}, label = { Text(stringResource(R.string.ai_tool_used, item.name)) },
                                    leadingIcon = { Icon(if (item.ok) Icons.Outlined.Build else Icons.Outlined.ErrorOutline, null, Modifier.size(18.dp)) })
                                is SuggItem -> SuggestionCard(item.s, onAccept = { answer(item.s, true) }, onReject = { answer(item.s, false) })
                                is NeedItem -> NeedCard(item.need) { grantFor(item.need) }
                            }
                        }
                        if (streaming) item(key = "progress") { LinearProgressIndicator(Modifier.fillMaxWidth()) }
                    }
                }
            }
            Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    input, { input = it }, Modifier.weight(1f), maxLines = 5, placeholder = { Text(stringResource(R.string.ai_input_hint)) },
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                )
                if (streaming) FilledTonalIconButton(onClick = { job?.cancel() }) { Icon(Icons.Outlined.Stop, stringResource(R.string.ai_stop)) }
                else FilledIconButton(onClick = { send() }, enabled = input.isNotBlank() && loaded) { Icon(Icons.AutoMirrored.Filled.Send, stringResource(R.string.send)) }
            }
        }
    }

    if (sheet) GrantsSheet(
        state = state, convId = id, grants = grants, study = study,
        onChanged = { refreshGrants() }, onError = { err(it) }, onDismiss = { sheet = false },
    )
}

@Composable
private fun Bubble(m: MsgItem) {
    val cs = MaterialTheme.colorScheme
    Box(Modifier.fillMaxWidth(), contentAlignment = if (m.user) Alignment.CenterEnd else Alignment.CenterStart) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = when { m.error -> cs.errorContainer; m.user -> cs.primaryContainer; else -> cs.surfaceContainerHigh },
            modifier = Modifier.widthIn(max = 520.dp).fillMaxWidth(if (m.user) 0.85f else 1f),
        ) {
            Box(Modifier.padding(12.dp)) {
                if (m.user || m.error) Text(m.text, style = MaterialTheme.typography.bodyMedium) else ChatMarkdown(m.text)
            }
        }
    }
}

@Composable
private fun SuggestionCard(s: AiSuggestion, onAccept: () -> Unit, onReject: () -> Unit) {
    val add = Color(0xFF2E7D32).copy(alpha = 0.18f)
    val del = MaterialTheme.colorScheme.error.copy(alpha = 0.18f)
    OutlinedCard(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(stringResource(R.string.ai_suggestion_title), style = MaterialTheme.typography.titleSmall)
            Column(Modifier.fillMaxWidth().heightIn(max = 280.dp).verticalScroll(rememberScrollState())) {
                s.diff.forEach { l ->
                    val bg = when (l.op) { "+" -> add; "-" -> del; else -> Color.Transparent }
                    Text(
                        (if (l.op == "+" || l.op == "-") l.op + " " else "  ") + l.text, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.fillMaxWidth().background(bg),
                    )
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = onAccept) { Text(stringResource(R.string.ai_accept)) }
                OutlinedButton(onClick = onReject) { Text(stringResource(R.string.ai_reject)) }
            }
        }
    }
}

@Composable
private fun NeedCard(n: AiGrantNeed, onAllow: () -> Unit) {
    val text = when (n.kind) {
        "page" -> stringResource(R.string.ai_need_page, n.title, modeLabel(n.operation ?: "append"))
        "study_update" -> stringResource(R.string.ai_need_study_update, n.title)
        else -> stringResource(R.string.ai_need_study_create)
    }
    ElevatedCard(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(text, style = MaterialTheme.typography.bodyMedium)
            Button(onClick = onAllow) { Text(stringResource(R.string.ai_allow)) }
        }
    }
}

@Composable
private fun modeLabel(mode: String) = stringResource(when (mode) { "replace" -> R.string.ai_mode_replace; "append" -> R.string.ai_mode_append; else -> R.string.ai_mode_suggest })

/** Autorisations d'écriture de la conversation (pages et entraînements), durée limitée et révocables. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun GrantsSheet(state: AppState, convId: String, grants: List<AiGrant>, study: List<AiStudyGrant>, onChanged: () -> Unit, onError: (Throwable) -> Unit, onDismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    val ws = (state.session as? Session.Ready)?.workspaceId
    var page by remember { mutableStateOf<Page?>(null) }
    var mode by remember { mutableStateOf("suggest") }
    var minutes by remember { mutableIntStateOf(60) }
    var pickPage by remember { mutableStateOf(false) }
    var pickSet by remember { mutableStateOf<List<AiStudySet>?>(null) }
    fun run(block: suspend () -> Unit) {
        scope.launch { runCatching { block() }.onSuccess { onChanged() }.onFailure { if (it is CancellationException) throw it; onError(it) } }
    }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(bottom = 24.dp)) {
            SectionTitle(stringResource(R.string.ai_grants))
            if (grants.isEmpty() && study.isEmpty()) Text(stringResource(R.string.ai_grants_none), Modifier.padding(horizontal = 20.dp, vertical = 8.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
            grants.forEach { g ->
                ListItem(
                    headlineContent = { Text(g.page_title.ifBlank { stringResource(R.string.untitled) }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = { Text(modeLabel(g.mode) + " · " + stringResource(R.string.ai_grant_until, untilLabel(g.expires_at))) },
                    trailingContent = { TextButton(onClick = { run { state.api.send("DELETE", "$AI/conversations/$convId/grants/${g.id}") } }) { Text(stringResource(R.string.revoke)) } },
                    colors = sheetItemColors(),
                )
            }
            study.forEach { g ->
                val label = if (g.kind == "update") stringResource(R.string.ai_study_update_label, g.set_title.orEmpty())
                else stringResource(R.string.ai_study_create_label, if (g.personal) stringResource(R.string.ai_study_personal_space) else g.workspace_name.orEmpty())
                ListItem(
                    headlineContent = { Text(label, maxLines = 2) },
                    supportingContent = { Text(stringResource(R.string.ai_grant_until, untilLabel(g.expires_at))) },
                    trailingContent = { TextButton(onClick = { run { state.api.send("DELETE", "$AI/conversations/$convId/study-grants/${g.id}") } }) { Text(stringResource(R.string.revoke)) } },
                    colors = sheetItemColors(),
                )
            }

            SectionTitle(stringResource(R.string.ai_duration))
            FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                DURATIONS.forEach { (label, m) -> FilterChip(selected = minutes == m, onClick = { minutes = m }, label = { Text(label) }) }
            }

            SectionTitle(stringResource(R.string.ai_grant_page))
            ListItem(
                headlineContent = { Text(page?.title?.ifBlank { null } ?: stringResource(R.string.ai_pick_page)) },
                leadingContent = { Icon(Icons.Outlined.Description, null) },
                modifier = Modifier.clickable { state.loadPages(); pickPage = true }, colors = sheetItemColors(),
            )
            FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                listOf("suggest", "append", "replace").forEach { m -> FilterChip(selected = mode == m, onClick = { mode = m }, label = { Text(modeLabel(m)) }) }
            }
            Button(
                enabled = page != null, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                onClick = {
                    val p = page ?: return@Button
                    run { state.api.send("POST", "$AI/conversations/$convId/grants", buildJsonObject { put("page_id", p.id); put("mode", mode); put("expires_in_minutes", minutes) }); page = null }
                },
            ) { Text(stringResource(R.string.ai_grant_page)) }

            SectionTitle(stringResource(R.string.ai_study_grants))
            ListItem(
                headlineContent = { Text(stringResource(R.string.ai_study_create_personal)) }, leadingContent = { Icon(Icons.Outlined.School, null) }, colors = sheetItemColors(),
                modifier = Modifier.clickable { run { state.api.send("POST", "$AI/conversations/$convId/study-grants", buildJsonObject { put("personal", true); put("expires_in_minutes", minutes) }) } },
            )
            if (ws != null) ListItem(
                headlineContent = { Text(stringResource(R.string.ai_study_create_workspace)) }, leadingContent = { Icon(Icons.Outlined.School, null) }, colors = sheetItemColors(),
                modifier = Modifier.clickable { run { state.api.send("POST", "$AI/conversations/$convId/study-grants", buildJsonObject { put("workspace_id", ws); put("expires_in_minutes", minutes) }) } },
            )
            ListItem(
                headlineContent = { Text(stringResource(R.string.ai_study_update)) }, leadingContent = { Icon(Icons.Outlined.Edit, null) }, colors = sheetItemColors(),
                modifier = Modifier.clickable {
                    scope.launch {
                        runCatching { state.api.get<List<AiStudySet>>("/api/study/sets" + if (ws != null) "?workspace_id=$ws" else "") }
                            .onSuccess { pickSet = it }.onFailure { if (it is CancellationException) throw it; onError(it) }
                    }
                },
            )
        }
    }
    if (pickPage) AlertDialog(
        onDismissRequest = { pickPage = false },
        title = { Text(stringResource(R.string.ai_pick_page)) },
        text = {
            val pages = state.pages.data.orEmpty().filter { it.can_edit != false }
            LazyColumn(Modifier.heightIn(max = 400.dp)) {
                items(pages, key = { it.id }) { p ->
                    ListItem(headlineContent = { Text(p.title.ifBlank { stringResource(R.string.untitled) }, maxLines = 1, overflow = TextOverflow.Ellipsis) }, colors = sheetItemColors(),
                        modifier = Modifier.clickable { page = p; pickPage = false })
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = { pickPage = false }) { Text(stringResource(R.string.cancel)) } },
    )
    pickSet?.let { sets ->
        AlertDialog(
            onDismissRequest = { pickSet = null },
            title = { Text(stringResource(R.string.ai_pick_set)) },
            text = {
                LazyColumn(Modifier.heightIn(max = 400.dp)) {
                    items(sets, key = { it.id }) { s ->
                        ListItem(headlineContent = { Text(s.title.ifBlank { stringResource(R.string.untitled) }, maxLines = 1, overflow = TextOverflow.Ellipsis) }, colors = sheetItemColors(),
                            modifier = Modifier.clickable {
                                pickSet = null
                                run { state.api.send("POST", "$AI/conversations/$convId/study-grants", buildJsonObject { put("set_id", s.id); put("expires_in_minutes", minutes) }) }
                            })
                    }
                }
            },
            confirmButton = {},
            dismissButton = { TextButton(onClick = { pickSet = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}
