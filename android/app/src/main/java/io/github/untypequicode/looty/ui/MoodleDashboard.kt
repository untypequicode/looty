package io.github.untypequicode.looty.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.*
import kotlinx.coroutines.launch

/** Les panneaux du tableau de bord quand le membre n'a rien changé (même ordre que sur le web). */
private val DEFAULT_PANELS = listOf(
    BoardPanel("deadlines", "deadlines", 0, 0), BoardPanel("courses", "courses", 5, 0),
    BoardPanel("messages", "messages", 0, 12), BoardPanel("notifications", "notifications", 6, 12),
)

/** Onglets de l'écran Moodle vers lesquels « Tout voir » mène. */
const val TAB_DEADLINES = 1
const val TAB_COURSES = 2
const val TAB_MESSAGES = 3
const val TAB_NOTIFICATIONS = 4

/**
 * Tableau de bord Moodle sur Android : les panneaux du membre empilés dans l'ordre de lecture
 * de la grille du web (de haut en bas, puis de gauche à droite). Il se règle depuis le web.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MoodleDashboard(state: AppState, open: (String?) -> Unit, goTo: (Int) -> Unit, onCourse: (Long, String) -> Unit, onConversation: (Long, String) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var courses by remember { mutableStateOf<List<MoodleCourse>?>(null) }
    var deadlines by remember { mutableStateOf<List<MoodleDeadline>>(emptyList()) }
    var conversations by remember { mutableStateOf<List<MoodleConversation>>(emptyList()) }
    var notifs by remember { mutableStateOf<List<MoodleNotif>>(emptyList()) }
    var error by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(false) }
    fun reload() {
        scope.launch {
            loading = true
            state.loadCoursePrefs()
            state.moodleDeadlines().onSuccess { deadlines = it }
            state.moodleConversations().onSuccess { conversations = it }
            state.moodleNotifications().onSuccess { notifs = it.notifications }
            state.moodleCourses().onSuccess { courses = it; error = null }.onFailure { error = errorText(ctx, it) }
            loading = false
        }
    }
    LaunchedEffect(Unit) { reload() }
    val list = courses
    val book = remember(list, state.coursePrefs) { CourseBook(list.orEmpty(), state.coursePrefs) }
    val panels = remember(state.boardPanels) { (state.boardPanels ?: DEFAULT_PANELS).sortedWith(compareBy({ it.y }, { it.x })) }
    val now = System.currentTimeMillis() / 1000
    val hiddenIds = remember(list) { list.orEmpty().filter { it.hidden }.map { it.id }.toSet() }

    PullToRefreshBox(isRefreshing = loading && list != null, onRefresh = { reload() }, modifier = Modifier.fillMaxSize()) {
        when {
            list == null && error != null -> EmptyState(Icons.Outlined.CloudOff, error!!)
            list == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(12.dp, 4.dp, 12.dp, 32.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                items(panels, key = { it.i }) { p ->
                    val seeAll = when (p.kind) { "deadlines" -> TAB_DEADLINES; "courses" -> TAB_COURSES; "messages" -> TAB_MESSAGES; "notifications" -> TAB_NOTIFICATIONS; else -> null }
                    val title = p.title?.takeIf { it.isNotBlank() } ?: when (p.kind) {
                        "deadlines" -> stringResource(R.string.moodle_deadlines)
                        "courses" -> stringResource(R.string.moodle_courses)
                        "messages" -> stringResource(R.string.moodle_messages)
                        "notifications" -> stringResource(R.string.notifications)
                        "folder" -> p.folder?.let { book.pathOf(it) }.orEmpty()
                        "course" -> book.byId(p.course)?.let { book.nameOf(it) }.orEmpty()
                        else -> stringResource(R.string.crs_sort_progress)
                    }
                    ElevatedCard(Modifier.fillMaxWidth()) {
                        Row(Modifier.padding(start = 16.dp, end = 4.dp, top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(title, Modifier.weight(1f), style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            if (seeAll != null) TextButton(onClick = { goTo(seeAll) }) { Text(stringResource(R.string.dash_see_all)) }
                        }
                        when (p.kind) {
                            "deadlines" -> {
                                val shown = deadlines.filter { p.opts?.includeHidden == true || it.course_id !in hiddenIds }.take(5)
                                if (shown.isEmpty()) Nothing(R.string.moodle_no_deadlines)
                                shown.forEach { d ->
                                    ListItem(
                                        headlineContent = { Text(d.name.orEmpty(), maxLines = 2, overflow = TextOverflow.Ellipsis) },
                                        supportingContent = { Text(listOfNotNull(d.course, d.due?.let { mailFullDate(it) }).joinToString(" · "), color = if (d.overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant) },
                                        leadingContent = { Icon(if (d.module == "quiz") Icons.Outlined.Quiz else Icons.Outlined.Assignment, null, tint = if (d.overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary) },
                                        colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                                        modifier = Modifier.clickable { open(d.url) },
                                    )
                                }
                            }
                            "courses", "folder" -> {
                                val scope2 = p.folder?.takeIf { p.kind == "folder" }?.let { book.treeOf(it).toSet() }
                                val filter = if (scope2 != null) "all" else p.opts?.filter ?: "progress"
                                val shown = book.courses.filter { matches(it, filter, now) && (scope2 == null || book.folderOf(it.id) in scope2) }
                                    .sortedByDescending { it.lastaccess ?: 0L }.take(6)
                                if (shown.isEmpty()) Nothing(R.string.moodle_no_courses)
                                shown.forEach { c -> CourseRow(c, book, onClick = { onCourse(c.id, book.nameOf(c)) }, onMenu = null, compact = true) }
                            }
                            "course" -> {
                                val c = book.byId(p.course)
                                if (c == null) Nothing(R.string.dash_pick_course) else CourseRow(c, book, onClick = { onCourse(c.id, book.nameOf(c)) }, onMenu = null)
                            }
                            "progress" -> {
                                val shown = book.courses.filter { !it.hidden && it.progress != null }.sortedBy { it.progress }.take(6)
                                if (shown.isEmpty()) Nothing(R.string.moodle_no_courses)
                                shown.forEach { c ->
                                    ListItem(
                                        headlineContent = { Text(book.nameOf(c), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                        supportingContent = { LinearProgressIndicator({ ((c.progress ?: 0.0) / 100).toFloat().coerceIn(0f, 1f) }, Modifier.fillMaxWidth().padding(top = 4.dp)) },
                                        trailingContent = { Text("${(c.progress ?: 0.0).toInt()} %", style = MaterialTheme.typography.labelMedium) },
                                        colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                                        modifier = Modifier.clickable { onCourse(c.id, book.nameOf(c)) },
                                    )
                                }
                            }
                            "messages" -> {
                                val shown = conversations.take(3)
                                if (shown.isEmpty()) Nothing(R.string.moodle_no_messages)
                                shown.forEach { c ->
                                    ListItem(
                                        headlineContent = { Text(c.name, fontWeight = if (c.unread > 0) FontWeight.Bold else null, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                        supportingContent = { Text(c.last_text.orEmpty(), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                        leadingContent = { Avatar(c.name, 36.dp) },
                                        trailingContent = { if (c.unread > 0) Badge { Text("${c.unread}") } },
                                        colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                                        modifier = Modifier.clickable { onConversation(c.id, c.name) },
                                    )
                                }
                            }
                            "notifications" -> {
                                val shown = notifs.take(3)
                                if (shown.isEmpty()) Nothing(R.string.inbox_empty)
                                shown.forEach { n ->
                                    ListItem(
                                        headlineContent = { Text(n.subject.orEmpty(), fontWeight = if (!n.read) FontWeight.Bold else null, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                                        leadingContent = { Icon(Icons.Outlined.Notifications, null, tint = if (n.read) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary) },
                                        colors = ListItemDefaults.colors(containerColor = androidx.compose.ui.graphics.Color.Transparent),
                                        modifier = Modifier.clickable(enabled = n.url != null) { open(n.url) },
                                    )
                                }
                            }
                        }
                        Spacer(Modifier.height(4.dp))
                    }
                }
            }
        }
    }
}

@Composable
private fun Nothing(text: Int) {
    Text(stringResource(text), Modifier.padding(16.dp, 4.dp, 16.dp, 12.dp), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
