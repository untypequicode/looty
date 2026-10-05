package io.github.untypequicode.looty.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.clickable
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.focus.focusRequester
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.BoardFull
import io.github.untypequicode.looty.api.Task
import kotlinx.coroutines.launch
import java.time.LocalDate

/** Couleur d'une étiquette (#rrggbb). */
fun labelColor(hex: String): Color = runCatching { Color(android.graphics.Color.parseColor(hex.take(7))) }.getOrDefault(Color(0xFF5E6AD2))

/** Tâche terminée (pour moi, si validation individuelle). */
fun Task.doneFor(me: String?): Boolean = if (is_per_person) me != null && me in validations || is_completed else is_completed

/**
 * Tableau : une colonne par page (balayer pour changer), onglets avec le nombre de
 * tâches ; appui long sur une carte : déplacer, terminer, supprimer.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun BoardScreen(state: AppState, boardId: String, onBack: () -> Unit, onTask: (String) -> Unit) {
    val ctx = LocalContext.current
    val entry = state.boardData[boardId]
    val board = entry?.data
    LaunchedEffect(boardId) { state.loadBoard(boardId) }
    var showDone by remember { mutableStateOf(false) }
    var onlyMine by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var actionsFor by remember { mutableStateOf<Task?>(null) }
    var createIn by remember { mutableStateOf<String?>(null) }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val me = state.me?.id
    val columns = board?.columns.orEmpty().sortedBy { it.position }
    val pager = rememberPagerState { columns.size }
    val deletedMsg = stringResource(R.string.task_deleted)
    val undo = stringResource(R.string.undo)

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(board?.name ?: "", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.FilterList, stringResource(R.string.filters)) }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.show_completed)) }, onClick = { showDone = !showDone; menu = false },
                            leadingIcon = { Checkbox(checked = showDone, onCheckedChange = null) })
                        DropdownMenuItem(text = { Text(stringResource(R.string.only_mine)) }, onClick = { onlyMine = !onlyMine; menu = false },
                            leadingIcon = { Checkbox(checked = onlyMine, onCheckedChange = null) })
                        HorizontalDivider()
                        DropdownMenuItem(text = { Text(stringResource(R.string.open_in_browser_view)) }, leadingIcon = { Icon(Icons.Outlined.OpenInNew, null) },
                            onClick = { menu = false; state.ws?.let { openWeb(ctx, Paths.board(it, boardId)) } })
                    }
                },
            )
        },
        floatingActionButton = {
            if (columns.isNotEmpty()) FloatingActionButton(onClick = { createIn = columns.getOrNull(pager.currentPage)?.id }) { Icon(Icons.Filled.Add, stringResource(R.string.new_task)) }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            if (board == null) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { if (entry?.error != null) Text(stringResource(R.string.error_generic)) else LoadingIndicatorCompat() }
                return@Column
            }
            fun visible(t: Task) = t.parent_id == null && (showDone || !t.doneFor(me)) && (!onlyMine || me in t.assignees)
            PrimaryScrollableTabRow(selectedTabIndex = pager.currentPage.coerceAtMost((columns.size - 1).coerceAtLeast(0)), edgePadding = 12.dp) {
                columns.forEachIndexed { i, c ->
                    Tab(selected = pager.currentPage == i, onClick = { scope.launch { pager.animateScrollToPage(i) } },
                        text = { Text("${c.name} · ${c.tasks.count(::visible)}", maxLines = 1) })
                }
            }
            HorizontalPager(pager, Modifier.fillMaxSize(), beyondViewportPageCount = 1) { page ->
                val col = columns[page]
                val tasks = col.tasks.filter(::visible).sortedBy { it.position }
                PullToRefreshBox(isRefreshing = entry.loading, onRefresh = { state.loadBoard(boardId) }) {
                    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(12.dp, 12.dp, 12.dp, 96.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (tasks.isEmpty()) item { EmptyState(Icons.Outlined.Inbox, stringResource(R.string.column_empty)) }
                        items(tasks, key = { it.id }) { t ->
                            TaskCard(state, board, t, onClick = { onTask(t.id) }, onLongClick = { actionsFor = t }, modifier = Modifier.animateItem())
                        }
                    }
                }
            }
        }
    }

    actionsFor?.let { t ->
        TaskActionsSheet(state, board ?: return@let, t, onDismiss = { actionsFor = null }, onDeleted = {
            scope.launch { if (snackbar.showSnackbar(deletedMsg, actionLabel = undo) == SnackbarResult.ActionPerformed) state.restoreTask(t.id) }
        })
    }
    createIn?.let { colId ->
        NewTaskInColumnSheet(onDismiss = { createIn = null }) { title ->
            scope.launch { state.createInColumn(boardId, colId, title); createIn = null }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
fun TaskCard(state: AppState, board: BoardFull, t: Task, onClick: () -> Unit, onLongClick: () -> Unit, modifier: Modifier = Modifier) {
    val haptics = LocalHapticFeedback.current
    val me = state.me?.id
    val done = t.doneFor(me)
    val subtasks = board.allTasks.filter { it.parent_id == t.id }
    val due = localDate(t.due_date)
    ElevatedCard(
        modifier = modifier.fillMaxWidth().clip(CardDefaults.elevatedShape)
            .combinedClickable(onClick = onClick, onLongClick = { haptics.performHapticFeedback(HapticFeedbackType.LongPress); onLongClick() }),
    ) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (t.labels.isNotEmpty()) {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    t.labels.take(4).forEach { l ->
                        Text(l.name, style = MaterialTheme.typography.labelSmall, color = Color.White, maxLines = 1,
                            modifier = Modifier.clip(RoundedCornerShape(6.dp)).background(labelColor(l.color)).padding(horizontal = 6.dp, vertical = 2.dp))
                    }
                }
            }
            Row(verticalAlignment = Alignment.Top) {
                PriorityIcon(t.priority)
                Text(t.title, style = MaterialTheme.typography.bodyLarge, maxLines = 3, overflow = TextOverflow.Ellipsis,
                    textDecoration = if (done) TextDecoration.LineThrough else null, modifier = Modifier.weight(1f))
                if (done) Icon(Icons.Filled.CheckCircle, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                if (due != null) {
                    val overdue = !done && due.isBefore(LocalDate.now())
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.Event, null, Modifier.size(14.dp), tint = if (overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.width(3.dp))
                        Text(dayLabel(due), style = MaterialTheme.typography.labelMedium, color = if (overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                if (subtasks.isNotEmpty()) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.Checklist, null, Modifier.size(14.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.width(3.dp))
                        Text("${subtasks.count { it.doneFor(me) }}/${subtasks.size}", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                Spacer(Modifier.weight(1f))
                Row(horizontalArrangement = Arrangement.spacedBy((-6).dp)) {
                    t.assignees.take(3).forEach { a -> Avatar(state.members[a]?.name, 22.dp) }
                }
            }
        }
    }
}

/** Priorité : 1 urgente, 2 haute, 3 moyenne, 4 basse. */
@Composable
fun PriorityIcon(p: Int) {
    if (p == 0) return
    val (icon, tint) = when (p) {
        1 -> Icons.Outlined.Error to MaterialTheme.colorScheme.error
        2 -> Icons.Outlined.KeyboardDoubleArrowUp to Color(0xFFEB6834)
        3 -> Icons.Outlined.KeyboardArrowUp to Color(0xFFEDA100)
        else -> Icons.Outlined.KeyboardArrowDown to MaterialTheme.colorScheme.onSurfaceVariant
    }
    Icon(icon, null, Modifier.size(20.dp).padding(end = 4.dp), tint = tint)
}

/** Appui long : déplacer vers une colonne, terminer / rouvrir, ouvrir, supprimer. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TaskActionsSheet(state: AppState, board: BoardFull, t: Task, onDismiss: () -> Unit, onDeleted: () -> Unit) {
    val me = state.me?.id
    // Ouvert en entier, défilant : « Supprimer » reste accessible même avec beaucoup de colonnes.
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        NoNavScrim()
        Column(Modifier.verticalScroll(androidx.compose.foundation.rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
            Text(t.title, style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            ListItem(
                colors = sheetItemColors(),
                headlineContent = { Text(stringResource(if (t.doneFor(me)) R.string.reopen else R.string.mark_done)) },
                leadingContent = { Icon(if (t.doneFor(me)) Icons.Outlined.RemoveDone else Icons.Outlined.TaskAlt, null) },
                modifier = Modifier.clickable(onClick = { state.toggleDone(t); onDismiss() }),
            )
            Text(stringResource(R.string.move_to), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            board.columns.sortedBy { it.position }.forEach { c ->
                ListItem(
                    colors = sheetItemColors(),
                    headlineContent = { Text(c.name) },
                    leadingContent = { RadioButton(selected = c.id == t.column_id, onClick = null) },
                    modifier = Modifier.clickable(onClick = { if (c.id != t.column_id) state.moveTask(t, c.id); onDismiss() }),
                )
            }
            HorizontalDivider(Modifier.padding(vertical = 8.dp))
            ListItem(
                colors = sheetItemColors(),
                headlineContent = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                leadingContent = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) },
                modifier = Modifier.clickable(onClick = { state.deleteTask(t); onDismiss(); onDeleted() }),
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NewTaskInColumnSheet(onDismiss: () -> Unit, onCreate: (String) -> Unit) {
    var title by remember { mutableStateOf("") }
    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Row(Modifier.padding(horizontal = 20.dp).padding(bottom = 24.dp).imePadding(), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = title, onValueChange = { title = it }, singleLine = true, label = { Text(stringResource(R.string.task_title)) },
                modifier = Modifier.weight(1f).focusRequester(focus),
                keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = androidx.compose.ui.text.input.ImeAction.Done),
                keyboardActions = androidx.compose.foundation.text.KeyboardActions(onDone = { if (title.isNotBlank()) onCreate(title) }),
            )
            Spacer(Modifier.width(8.dp))
            FilledIconButton(onClick = { if (title.isNotBlank()) onCreate(title) }, enabled = title.isNotBlank()) { Icon(Icons.Filled.Add, stringResource(R.string.create)) }
        }
    }
}

