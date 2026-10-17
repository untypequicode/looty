package io.github.untypequicode.looty.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Article
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.FeedItem
import io.github.untypequicode.looty.api.Trash
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonPrimitive
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

@Composable
private fun feedVerb(i: FeedItem): String = stringResource(
    when {
        i.source == "comment" -> R.string.feed_commented
        i.source == "page" -> when (i.type) { "create" -> R.string.feed_page_create; "rename" -> R.string.feed_page_rename; "move" -> R.string.feed_page_move
            "delete" -> R.string.feed_page_delete; "restore" -> R.string.feed_page_restore; else -> R.string.feed_page_edit }
        else -> when (i.type) {
            "create" -> R.string.feed_task_created; "move_column" -> R.string.feed_task_moved
            "toggle_validation" -> if ((i.new_value as? JsonObject)?.get("is_completed")?.let { runCatching { it.jsonPrimitive.booleanOrNull }.getOrNull() } == true) R.string.feed_task_completed else R.string.feed_task_reopened
            "add_assignee" -> R.string.feed_task_assigned; "remove_assignee" -> R.string.feed_task_unassigned
            "add_label", "remove_label" -> R.string.feed_task_labeled; "upload_attachment" -> R.string.feed_task_attached
            "update_title" -> R.string.feed_task_renamed; "update_description" -> R.string.feed_task_described
            "update_priority" -> R.string.feed_task_prioritized; "update_due_date" -> R.string.feed_task_scheduled
            else -> R.string.feed_task_updated
        }
    },
)

private fun dayOf(iso: String) = runCatching { Instant.parse(iso).atZone(ZoneId.systemDefault()).toLocalDate() }.getOrNull()
private fun hhmm(iso: String) = runCatching { Instant.parse(iso).atZone(ZoneId.systemDefault()).format(DateTimeFormatter.ofPattern("HH:mm")) }.getOrDefault("")

@Composable
private fun dayTitle(d: LocalDate): String = when (d) {
    LocalDate.now() -> stringResource(R.string.today)
    LocalDate.now().minusDays(1) -> stringResource(R.string.yesterday)
    else -> d.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.FULL))
}

/** Fil d'activité de l'espace (ou d'une personne), regroupé par jour. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ActivityScreen(state: AppState, onBack: () -> Unit, onRoute: (String) -> Unit) {
    var who by remember { mutableStateOf<String?>(null) }
    var items by remember { mutableStateOf<List<FeedItem>?>(null) }
    var hasMore by remember { mutableStateOf(false) }
    var loading by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val list = rememberLazyListState()
    fun load(more: Boolean) {
        if (loading) return
        loading = true
        scope.launch {
            val r = state.feed(who, if (more) items?.lastOrNull()?.created_at else null)
            if (r != null) { items = if (more) items.orEmpty() + r.items else r.items; hasMore = r.has_more }
            loading = false
        }
    }
    LaunchedEffect(who) { items = null; load(false) }
    val atEnd by remember { derivedStateOf { list.layoutInfo.visibleItemsInfo.lastOrNull()?.index?.let { it >= list.layoutInfo.totalItemsCount - 4 } ?: false } }
    LaunchedEffect(atEnd, hasMore) { if (atEnd && hasMore) load(true) }
    Scaffold(topBar = {
        TopAppBar(
            title = { Text(stringResource(R.string.activity)) },
            navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            actions = {
                Box {
                    TextButton(onClick = { menu = true }) { Text(who?.let { state.members[it]?.name } ?: stringResource(R.string.everyone), maxLines = 1); Icon(Icons.Filled.ArrowDropDown, null) }
                    DropdownMenu(menu, { menu = false }) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.everyone)) }, leadingIcon = { RadioButton(who == null, null) }, onClick = { menu = false; who = null })
                        state.members.values.sortedBy { it.name }.forEach { m ->
                            DropdownMenuItem(text = { Text(m.name) }, leadingIcon = { RadioButton(who == m.user_id, null) }, onClick = { menu = false; who = m.user_id })
                        }
                    }
                }
            },
        )
    }) { padding ->
        PullToRefreshBox(isRefreshing = loading && items != null, onRefresh = { load(false) }, modifier = Modifier.padding(padding).fillMaxSize()) {
            val l = items
            if (l == null) { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }; return@PullToRefreshBox }
            LazyColumn(Modifier.fillMaxSize(), state = list, contentPadding = PaddingValues(bottom = 32.dp)) {
                if (l.isEmpty()) item { EmptyState(Icons.Outlined.History, stringResource(R.string.feed_empty)) }
                l.groupBy { dayOf(it.created_at) }.forEach { (day, group) ->
                    if (day != null) item(key = "d$day") { SectionTitle(dayTitle(day)) }
                    items(group, key = { it.source + it.id }) { i ->
                        val target = when {
                            i.source == "page" && i.type == "delete" -> "trash"
                            i.source == "page" -> "doc/${i.target_id}"
                            else -> "task/${i.target_id}"
                        }
                        val name = state.members[i.user_id]?.name ?: i.user_name ?: "?"
                        ListItem(
                            headlineContent = {
                                Text("$name ${feedVerb(i)} " + listOfNotNull(i.icon, i.title.ifBlank { stringResource(R.string.untitled) }).joinToString(" "), maxLines = 2, overflow = TextOverflow.Ellipsis)
                            },
                            supportingContent = {
                                Column {
                                    i.snippet?.takeIf { it.isNotBlank() }?.let { Text("“$it”", maxLines = 2, overflow = TextOverflow.Ellipsis) }
                                    Text(listOfNotNull(hhmm(i.created_at), i.board_name).joinToString(" · "), style = MaterialTheme.typography.labelSmall)
                                }
                            },
                            leadingContent = {
                                BadgedBox(badge = {
                                    Icon(when (i.source) { "page" -> Icons.AutoMirrored.Outlined.Article; "comment" -> Icons.Outlined.ChatBubbleOutline; else -> Icons.Outlined.TaskAlt }, null,
                                        Modifier.size(14.dp), tint = MaterialTheme.colorScheme.primary)
                                }) { Avatar(name, 34.dp) }
                            },
                            modifier = Modifier.clickable { onRoute(target) },
                        )
                    }
                }
                if (loading && l.isNotEmpty()) item { Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp)) } }
            }
        }
    }
}

/** Corbeille : tâches et documents supprimés, à restaurer ou supprimer définitivement. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TrashScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var data by remember { mutableStateOf<Trash?>(null) }
    var loading by remember { mutableStateOf(false) }
    var confirm by remember { mutableStateOf<Triple<String, String, String>?>(null) } // kind, id, title
    var emptyAll by remember { mutableStateOf(false) }
    val snackbar = remember { SnackbarHostState() }
    val restored = stringResource(R.string.restored)
    fun reload() { scope.launch { loading = true; data = state.trash() ?: data; loading = false } }
    LaunchedEffect(Unit) { reload() }
    val me = state.me?.id
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.trash)) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    val d = data
                    if (d != null && d.can_manage && (d.tasks.isNotEmpty() || d.pages.isNotEmpty())) TextButton(onClick = { emptyAll = true }) { Text(stringResource(R.string.empty_trash), color = MaterialTheme.colorScheme.error) }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        PullToRefreshBox(isRefreshing = loading && data != null, onRefresh = { reload() }, modifier = Modifier.padding(padding).fillMaxSize()) {
            val d = data
            if (d == null) { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { if (loading) LoadingIndicatorCompat() else Text(stringResource(R.string.error_generic)) }; return@PullToRefreshBox }
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
                item {
                    Text(if (d.retention_days > 0) pluralStringResource(R.plurals.trash_retention, d.retention_days, d.retention_days) else stringResource(R.string.trash_kept),
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(16.dp))
                }
                if (d.tasks.isEmpty() && d.pages.isEmpty()) item { EmptyState(Icons.Outlined.DeleteOutline, stringResource(R.string.trash_empty)) }
                @Composable
                fun TrashItem(kind: String, id: String, title: String, sub: String, icon: @Composable () -> Unit, deletedBy: String?) {
                    ListItem(
                        headlineContent = { Text(title.ifBlank { stringResource(R.string.untitled) }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = { Text(sub, maxLines = 2) },
                        leadingContent = icon,
                        trailingContent = {
                            Row {
                                IconButton(onClick = { scope.launch { if (state.restoreTrashed(kind, id)) { reload(); snackbar.showSnackbar(restored) } } }) { Icon(Icons.Outlined.RestoreFromTrash, stringResource(R.string.restore)) }
                                if (d.can_manage || deletedBy == me) IconButton(onClick = { confirm = Triple(kind, id, title) }) { Icon(Icons.Outlined.DeleteForever, stringResource(R.string.delete_forever), tint = MaterialTheme.colorScheme.error) }
                            }
                        },
                    )
                }
                if (d.pages.isNotEmpty()) item { SectionTitle(stringResource(R.string.nav_docs)) }
                items(d.pages, key = { "p" + it.id }) { p ->
                    TrashItem("pages", p.id, p.title, listOfNotNull(
                        p.parent_title?.let { stringResource(R.string.trash_in, it) },
                        if (p.subpages > 0) pluralStringResource(R.plurals.n_subpages, p.subpages.toInt(), p.subpages.toInt()) else null,
                        stringResource(R.string.deleted_by, p.deleted_by_name ?: "?", relativeTime(p.deleted_at, ctx)),
                    ).joinToString(" · "), { if (!p.icon.isNullOrBlank()) Text(p.icon) else Icon(Icons.AutoMirrored.Outlined.Article, null) }, p.deleted_by)
                }
                if (d.tasks.isNotEmpty()) item { SectionTitle(stringResource(R.string.nav_tasks)) }
                items(d.tasks, key = { "t" + it.id }) { t ->
                    TrashItem("tasks", t.id, t.title, listOfNotNull(
                        t.board_name,
                        if (t.subtasks > 0) pluralStringResource(R.plurals.n_subtasks, t.subtasks.toInt(), t.subtasks.toInt()) else null,
                        stringResource(R.string.deleted_by, t.deleted_by_name ?: "?", relativeTime(t.deleted_at, ctx)),
                    ).joinToString(" · "), { Icon(Icons.Outlined.TaskAlt, null) }, t.deleted_by)
                }
            }
        }
    }
    confirm?.let { (kind, id, title) ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text(stringResource(R.string.delete_forever_title, title.ifBlank { stringResource(R.string.untitled) })) },
            text = { Text(stringResource(R.string.delete_forever_message)) },
            confirmButton = { TextButton(onClick = { confirm = null; scope.launch { if (state.deleteTrashed(kind, id)) reload() } }) { Text(stringResource(R.string.delete_forever), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    if (emptyAll) AlertDialog(
        onDismissRequest = { emptyAll = false },
        title = { Text(stringResource(R.string.empty_trash_title)) },
        text = { Text(stringResource(R.string.delete_forever_message)) },
        confirmButton = { TextButton(onClick = { emptyAll = false; scope.launch { if (state.emptyTrash()) reload() } }) { Text(stringResource(R.string.empty_trash), color = MaterialTheme.colorScheme.error) } },
        dismissButton = { TextButton(onClick = { emptyAll = false }) { Text(stringResource(R.string.cancel)) } },
    )
}
