package io.github.untypequicode.looty.ui

import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.MoodleLoginActivity
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.*
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private fun openExternal(ctx: android.content.Context, url: String) {
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

/** Moodle : échéances, cours, messages, notifications ; connexion du compte. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MoodleScreen(state: AppState, onBack: () -> Unit, onConversation: (Long, String) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var overview by remember { mutableStateOf<MoodleOverview?>(null) }
    var failed by remember { mutableStateOf(false) }
    var tab by remember { mutableIntStateOf(0) }
    var menu by remember { mutableStateOf(false) }
    var confirmDisconnect by remember { mutableStateOf(false) }
    val snackbar = remember { SnackbarHostState() }
    val synced = stringResource(R.string.moodle_calendar_synced)
    fun reload() { scope.launch { val o = state.moodleOverview(); if (o != null) overview = o else failed = true } }
    LaunchedEffect(Unit) { reload() }
    val conn = overview?.connection
    /** Lien Moodle ouvert dans le navigateur, déjà connecté si possible. */
    fun open(url: String?) = state.launch { state.moodleOpen(url)?.let { openExternal(ctx, it) } }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text("Moodle")
                        conn?.let { Text(listOfNotNull(it.site_name, it.full_name).joinToString(" · "), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                    }
                },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    if (conn != null) {
                        IconButton(onClick = { open(null) }) { Icon(Icons.Outlined.OpenInBrowser, stringResource(R.string.open_moodle)) }
                        IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                        DropdownMenu(menu, { menu = false }) {
                            DropdownMenuItem(text = { Text(stringResource(R.string.moodle_sync_calendar)) }, leadingIcon = { Icon(Icons.Outlined.EventRepeat, null) },
                                onClick = { menu = false; state.launch { if (state.moodleSyncCalendar()) snackbar.showSnackbar(synced) } })
                            DropdownMenuItem(text = { Text(stringResource(R.string.moodle_notify)) }, leadingIcon = { Checkbox(conn.notify, null) },
                                onClick = { menu = false; state.launch { if (state.moodleNotify(!conn.notify)) reload() } })
                            DropdownMenuItem(text = { Text(stringResource(R.string.moodle_disconnect), color = MaterialTheme.colorScheme.error) },
                                leadingIcon = { Icon(Icons.Outlined.LinkOff, null, tint = MaterialTheme.colorScheme.error) }, onClick = { menu = false; confirmDisconnect = true })
                        }
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            when {
                overview == null && !failed -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                overview == null -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic))
                conn == null -> MoodleConnect(state, overview!!) { reload() }
                else -> {
                    conn.last_error?.let { e ->
                        Surface(color = MaterialTheme.colorScheme.errorContainer, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth().padding(16.dp, 4.dp)) {
                            Text(if (e.startsWith("moodle_")) errorText(ctx, e) else e, color = MaterialTheme.colorScheme.onErrorContainer, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(12.dp))
                        }
                    }
                    PrimaryScrollableTabRow(selectedTabIndex = tab, edgePadding = 12.dp) {
                        listOf(R.string.moodle_deadlines, R.string.moodle_courses, R.string.moodle_messages, R.string.notifications).forEachIndexed { i, label ->
                            Tab(selected = tab == i, onClick = { tab = i }, text = { Text(stringResource(label), maxLines = 1) })
                        }
                    }
                    when (tab) {
                        0 -> MoodleList(load = { state.moodleDeadlines() }, empty = R.string.moodle_no_deadlines) { list: List<MoodleDeadline> ->
                            items(list, key = { it.id }) { d ->
                                ListItem(
                                    headlineContent = { Text(d.name.orEmpty(), maxLines = 2, overflow = TextOverflow.Ellipsis) },
                                    supportingContent = { Text(listOfNotNull(d.course, d.due?.let { mailFullDate(it) }).joinToString(" · "), color = if (d.overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant) },
                                    leadingContent = { Icon(if (d.module == "quiz") Icons.Outlined.Quiz else Icons.Outlined.Assignment, null, tint = if (d.overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary) },
                                    trailingContent = d.action?.let { { Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary) } },
                                    modifier = Modifier.clickable { open(d.url) },
                                )
                            }
                        }
                        1 -> MoodleList(load = { state.moodleCourses() }, empty = R.string.moodle_no_courses) { list: List<MoodleCourse> ->
                            items(list.sortedWith(compareBy({ !it.favourite }, { it.fullname.orEmpty() })), key = { it.id }) { c ->
                                ListItem(
                                    headlineContent = { Text(c.fullname.orEmpty(), maxLines = 2, overflow = TextOverflow.Ellipsis) },
                                    supportingContent = {
                                        Column {
                                            Text(listOfNotNull(c.shortname, c.category).joinToString(" · "), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                            c.progress?.let { p -> LinearProgressIndicator({ (p / 100).toFloat().coerceIn(0f, 1f) }, Modifier.fillMaxWidth().padding(top = 6.dp)) }
                                        }
                                    },
                                    leadingContent = { Icon(if (c.favourite) Icons.Outlined.Star else Icons.Outlined.School, null, tint = MaterialTheme.colorScheme.primary) },
                                    modifier = Modifier.clickable { open(c.url) },
                                )
                            }
                        }
                        2 -> MoodleList(load = { state.moodleConversations() }, empty = R.string.moodle_no_messages) { list: List<MoodleConversation> ->
                            items(list, key = { it.id }) { c ->
                                ListItem(
                                    headlineContent = { Text(c.name, fontWeight = if (c.unread > 0) FontWeight.Bold else null, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                    supportingContent = { Text((if (c.last_from_me == true) stringResource(R.string.you_prefix) else "") + c.last_text.orEmpty(), maxLines = 2, overflow = TextOverflow.Ellipsis) },
                                    leadingContent = { Avatar(c.name, 40.dp) },
                                    trailingContent = {
                                        Column(horizontalAlignment = Alignment.End) {
                                            c.last_time?.let { Text(relativeTime(it, ctx), style = MaterialTheme.typography.labelSmall) }
                                            if (c.unread > 0) Badge { Text("${c.unread}") }
                                        }
                                    },
                                    modifier = Modifier.clickable { onConversation(c.id, c.name) },
                                )
                            }
                        }
                        else -> MoodleList(load = { state.moodleNotifications().map { it.notifications } }, empty = R.string.inbox_empty) { list: List<MoodleNotif> ->
                            items(list, key = { it.id }) { n ->
                                ListItem(
                                    headlineContent = { Text(n.subject.orEmpty(), fontWeight = if (!n.read) FontWeight.Bold else null, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                                    supportingContent = { Text(listOfNotNull(n.text?.takeIf { it.isNotBlank() }, n.time?.let { relativeTime(it, ctx) }).joinToString("\n"), maxLines = 3, overflow = TextOverflow.Ellipsis) },
                                    leadingContent = { Icon(Icons.Outlined.Notifications, null, tint = if (n.read) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary) },
                                    modifier = Modifier.clickable(enabled = n.url != null) { open(n.url) },
                                )
                            }
                        }
                    }
                }
            }
        }
    }
    if (confirmDisconnect) AlertDialog(
        onDismissRequest = { confirmDisconnect = false },
        title = { Text(stringResource(R.string.moodle_disconnect_title)) },
        text = { Text(stringResource(R.string.moodle_disconnect_message)) },
        confirmButton = { TextButton(onClick = { confirmDisconnect = false; state.launch { if (state.moodleDisconnect()) reload() } }) { Text(stringResource(R.string.moodle_disconnect), color = MaterialTheme.colorScheme.error) } },
        dismissButton = { TextButton(onClick = { confirmDisconnect = false }) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Liste chargée depuis Moodle (à la demande), avec erreur lisible et « tirer pour actualiser ». */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun <T> MoodleList(load: suspend () -> Result<List<T>>, empty: Int, content: androidx.compose.foundation.lazy.LazyListScope.(List<T>) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<T>?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(false) }
    fun reload() { scope.launch { loading = true; load().onSuccess { list = it; error = null }.onFailure { error = errorText(ctx, it) }; loading = false } }
    LaunchedEffect(Unit) { reload() }
    PullToRefreshBox(isRefreshing = loading && list != null, onRefresh = { reload() }, modifier = Modifier.fillMaxSize()) {
        val l = list
        when {
            l == null && error != null -> EmptyState(Icons.Outlined.CloudOff, error!!)
            l == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
                if (l.isEmpty()) item { EmptyState(Icons.Outlined.Inbox, stringResource(empty)) }
                content(l)
            }
        }
    }
}

/**
 * Connexion du compte Moodle : par la page de connexion de l'université (SSO), ou par
 * identifiant et mot de passe pour les comptes locaux.
 */
@Composable
private fun MoodleConnect(state: AppState, overview: MoodleOverview, onConnected: () -> Unit) {
    val ctx = LocalContext.current
    var site by remember { mutableStateOf<MoodleSite?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    LaunchedEffect(Unit) { state.moodleSite().onSuccess { site = it }.onFailure { error = errorText(ctx, it) } }
    fun connect(body: kotlinx.serialization.json.JsonObject) {
        busy = true; error = null
        state.launch { state.moodleConnect(body).onSuccess { onConnected() }.onFailure { error = errorText(ctx, it) }; busy = false }
    }
    val login = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { r ->
        val link = if (r.resultCode == android.app.Activity.RESULT_OK) r.data?.getStringExtra(MoodleLoginActivity.EXTRA_LINK) else null
        if (link != null) connect(buildJsonObject { put("token", link) })
    }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Icon(Icons.Outlined.School, null, Modifier.size(48.dp), tint = MaterialTheme.colorScheme.primary)
        Text(stringResource(R.string.moodle_connect_title), style = MaterialTheme.typography.headlineSmall)
        Text(stringResource(R.string.moodle_connect_hint, site?.site_name ?: overview.url ?: overview.default_url), color = MaterialTheme.colorScheme.onSurfaceVariant)
        error?.let {
            Surface(color = MaterialTheme.colorScheme.errorContainer, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth()) {
                Text(it, color = MaterialTheme.colorScheme.onErrorContainer, modifier = Modifier.padding(12.dp))
            }
        }
        if (busy) LinearProgressIndicator(Modifier.fillMaxWidth())
        Button(enabled = site?.launch_url != null && !busy, onClick = {
            login.launch(Intent(ctx, MoodleLoginActivity::class.java).putExtra(MoodleLoginActivity.EXTRA_URL, site!!.launch_url))
        }, modifier = Modifier.fillMaxWidth()) { Icon(Icons.Outlined.Login, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.moodle_sign_in)) }
        HorizontalDivider()
        Text(stringResource(R.string.moodle_local_account), style = MaterialTheme.typography.titleSmall)
        OutlinedTextField(username, { username = it }, singleLine = true, label = { Text(stringResource(R.string.mail_username)) }, modifier = Modifier.fillMaxWidth())
        OutlinedTextField(password, { password = it }, singleLine = true, label = { Text(stringResource(R.string.mail_password)) }, modifier = Modifier.fillMaxWidth(),
            visualTransformation = PasswordVisualTransformation(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password))
        OutlinedButton(enabled = username.isNotBlank() && password.isNotEmpty() && !busy, onClick = { connect(buildJsonObject { put("username", username.trim()); put("password", password) }) },
            modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.moodle_connect)) }
    }
}

/** Conversation Moodle : messages et réponse. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MoodleConversationScreen(state: AppState, id: Long, name: String, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var thread by remember { mutableStateOf<MoodleThread?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var text by remember { mutableStateOf("") }
    var sending by remember { mutableStateOf(false) }
    val list = rememberLazyListState()
    fun reload() { scope.launch { state.moodleThread(id).onSuccess { thread = it }.onFailure { error = errorText(ctx, it) } } }
    LaunchedEffect(id) { reload() }
    LaunchedEffect(thread?.messages?.size) { thread?.messages?.size?.takeIf { it > 0 }?.let { list.scrollToItem(it - 1) } }
    Scaffold(
        topBar = {
            TopAppBar(title = { Text(name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } })
        },
        bottomBar = {
            Surface(tonalElevation = 3.dp) {
                Row(Modifier.navigationBarsPadding().imePadding().padding(12.dp, 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(text, { text = it }, placeholder = { Text(stringResource(R.string.reply_placeholder)) }, modifier = Modifier.weight(1f), maxLines = 5)
                    Spacer(Modifier.width(8.dp))
                    FilledIconButton(enabled = text.isNotBlank() && !sending, onClick = {
                        sending = true
                        scope.launch { if (state.moodleSend(id, text.trim())) { text = ""; reload() }; sending = false }
                    }) { Icon(Icons.AutoMirrored.Filled.Send, stringResource(R.string.send)) }
                }
            }
        },
    ) { padding ->
        val t = thread
        when {
            t == null && error != null -> Box(Modifier.padding(padding)) { EmptyState(Icons.Outlined.CloudOff, error!!) }
            t == null -> Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            else -> LazyColumn(Modifier.padding(padding).fillMaxSize(), state = list, contentPadding = PaddingValues(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(t.messages, key = { it.id }) { m ->
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = if (m.mine) Arrangement.End else Arrangement.Start) {
                        Column(Modifier.widthIn(max = 300.dp).clip(RoundedCornerShape(16.dp))
                            .background(if (m.mine) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh).padding(12.dp)) {
                            if (!m.mine) Text(m.from, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                            HtmlBlocks(state, m.html)
                            m.time?.let { Text(relativeTime(it, ctx), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                        }
                    }
                }
            }
        }
    }
}
