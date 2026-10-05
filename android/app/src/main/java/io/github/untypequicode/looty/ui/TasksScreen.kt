package io.github.untypequicode.looty.ui

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.outlined.Circle
import androidx.compose.material.icons.outlined.TaskAlt
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.MyTask
import java.time.LocalDate
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TasksScreen(state: AppState, padding: PaddingValues, snackbar: SnackbarHostState) {
    val list = state.tasks.data.orEmpty()
    // Message « Tâche terminée · Annuler » rattaché à l'écran (la ligne disparaît de la liste).
    val screenScope = rememberCoroutineScope()
    val today = LocalDate.now()
    val groups = listOf(
        stringResource(R.string.overdue) to list.filter { localDate(it.due_date)?.isBefore(today) == true },
        stringResource(R.string.today) to list.filter { localDate(it.due_date) == today },
        stringResource(R.string.this_week) to list.filter { localDate(it.due_date)?.let { d -> d.isAfter(today) && !d.isAfter(today.plusDays(7)) } == true },
        stringResource(R.string.later) to list.filter { localDate(it.due_date)?.isAfter(today.plusDays(7)) == true },
        stringResource(R.string.no_date) to list.filter { it.due_date == null },
    ).filter { it.second.isNotEmpty() }
    Column(Modifier.padding(padding)) {
        SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
            val scopes = listOf("mine" to stringResource(R.string.tasks_mine), "all" to stringResource(R.string.tasks_all))
            scopes.forEachIndexed { i, (key, label) ->
                SegmentedButton(selected = state.taskScope == key, onClick = { state.setScope(key) }, shape = SegmentedButtonDefaults.itemShape(i, scopes.size)) { Text(label, maxLines = 1) }
            }
        }
        PullToRefreshBox(isRefreshing = state.tasks.loading && state.tasks.data != null, onRefresh = { state.loadTasks() }) {
            if (state.tasks.data == null && state.tasks.loading) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            } else if (list.isEmpty()) {
                LazyColumn(Modifier.fillMaxSize()) { item { EmptyState(Icons.Outlined.TaskAlt, stringResource(R.string.no_tasks)) } }
            } else {
                LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
                    groups.forEach { (title, items) ->
                        item(key = "h$title") { SectionTitle("$title · ${items.size}") }
                        items(items, key = { it.id }) { t -> TaskRow(state, t, snackbar = snackbar, scope = screenScope) }
                    }
                }
            }
        }
    }
}

/** Ligne de tâche : cercle à cocher (avec annulation), titre, tableau · colonne, échéance. */
@Composable
fun TaskRow(state: AppState, t: MyTask, compact: Boolean = false, snackbar: SnackbarHostState? = null, scope: kotlinx.coroutines.CoroutineScope? = null) {
    val ctx = LocalContext.current
    val haptics = LocalHapticFeedback.current
    var done by remember(t.id, t.done_by_me) { mutableStateOf(t.done_by_me) }
    val doneMsg = stringResource(R.string.task_done)
    val undo = stringResource(R.string.undo)
    val rowScope = rememberCoroutineScope()
    val canCheck = t.assigned_to_me || !t.is_per_person
    val due = localDate(t.due_date)
    val overdue = due?.isBefore(LocalDate.now()) == true
    val tint by animateColorAsState(if (done) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline, label = "check")
    ListItem(
        headlineContent = { Text(t.title, maxLines = 2, overflow = TextOverflow.Ellipsis, textDecoration = if (done) TextDecoration.LineThrough else null) },
        supportingContent = {
            Text(
                if (t.done_by_me && !compact) "${stringResource(R.string.done_by_me)} · ${t.board_name}" else "${t.board_name} · ${t.column_name}",
                maxLines = 1, overflow = TextOverflow.Ellipsis,
            )
        },
        leadingContent = {
            if (canCheck) IconButton(onClick = {
                done = !done
                haptics.performHapticFeedback(HapticFeedbackType.Confirm)
                state.toggleTask(t.id)
                if (done && snackbar != null) (scope ?: rowScope).launchUndo(snackbar, doneMsg, undo) { state.toggleTask(t.id) }
            }) { Icon(if (done) Icons.Filled.CheckCircle else Icons.Outlined.Circle, null, tint = tint) }
        },
        trailingContent = due?.let { { Text(dayLabel(it), style = MaterialTheme.typography.labelMedium, color = if (overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant) } },
        modifier = Modifier.clickable { state.ws?.let { openWeb(ctx, Paths.task(it, t.board_id, t.id)) } },
    )
}

private fun kotlinx.coroutines.CoroutineScope.launchUndo(snackbar: SnackbarHostState, msg: String, action: String, onUndo: () -> Unit) {
    launch {
        if (snackbar.showSnackbar(msg, actionLabel = action, duration = SnackbarDuration.Short) == SnackbarResult.ActionPerformed) onUndo()
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LoadingIndicatorCompat() = LoadingIndicator()
