package io.github.untypequicode.looty.ui

import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.clickable
import androidx.compose.foundation.background
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.fromHtml
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Task
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset

private val PRIORITIES = listOf(R.string.prio_0, R.string.prio_1, R.string.prio_2, R.string.prio_3, R.string.prio_4)

/** Détail d'une tâche, entièrement natif. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun TaskScreen(state: AppState, taskId: String, onBack: () -> Unit, onTask: (String) -> Unit) {
    val ctx = LocalContext.current
    val haptics = LocalHapticFeedback.current
    LaunchedEffect(taskId) { state.ensureTask(taskId); state.loadComments(taskId) }
    val board = state.boardOf(taskId)
    val task = board?.task(taskId)
    val me = state.me?.id
    var menu by remember { mutableStateOf(false) }
    var pickDate by remember { mutableStateOf<String?>(null) }
    var assigneesSheet by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(listOfNotNull(board?.name, task?.let { board?.column(it.column_id)?.name }).joinToString(" · "),
                        style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.open_in_browser_view)) }, leadingIcon = { Icon(Icons.Outlined.OpenInNew, null) }, onClick = {
                            menu = false
                            val ws = state.ws ?: return@DropdownMenuItem
                            board?.let { openWeb(ctx, Paths.task(ws, it.id, taskId)) }
                        })
                        if (task != null) DropdownMenuItem(text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                            leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) },
                            onClick = { menu = false; state.deleteTask(task); onBack() })
                    }
                },
            )
        },
    ) { padding ->
        if (task == null || board == null) {
            Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            return@Scaffold
        }
        val done = task.doneFor(me)
        Column(Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).imePadding().padding(bottom = 32.dp)) {
            // Titre (enregistré en quittant le champ)
            var title by remember(task.id) { mutableStateOf(task.title) }
            TextField(
                value = title, onValueChange = { title = it }, textStyle = MaterialTheme.typography.headlineSmall,
                colors = TextFieldDefaults.colors(focusedContainerColor = MaterialTheme.colorScheme.surface, unfocusedContainerColor = MaterialTheme.colorScheme.surface,
                    focusedIndicatorColor = MaterialTheme.colorScheme.primary, unfocusedIndicatorColor = MaterialTheme.colorScheme.surface),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (title.isNotBlank() && title != task.title) state.updateTask(task, buildJsonObject { put("title", title.trim()) }) { it.copy(title = title.trim()) } }),
                modifier = Modifier.fillMaxWidth().padding(horizontal = 4.dp).onFocusChanged { f ->
                    if (!f.isFocused && title.isNotBlank() && title.trim() != task.title) state.updateTask(task, buildJsonObject { put("title", title.trim()) }) { it.copy(title = title.trim()) }
                },
            )
            // Terminer / rouvrir
            Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                if (done) FilledTonalButton(onClick = { haptics.performHapticFeedback(HapticFeedbackType.Confirm); state.toggleDone(task) }) {
                    Icon(Icons.Filled.CheckCircle, null); Spacer(Modifier.width(8.dp)); Text(stringResource(if (task.is_per_person) R.string.done_by_me else R.string.completed))
                } else Button(onClick = { haptics.performHapticFeedback(HapticFeedbackType.Confirm); state.toggleDone(task) }) {
                    Icon(Icons.Outlined.TaskAlt, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.mark_done))
                }
                if (task.is_per_person && task.assignees.isNotEmpty()) {
                    Spacer(Modifier.width(12.dp))
                    Text(stringResource(R.string.validations, task.validations.count { it in task.assignees }, task.assignees.size), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }

            SectionTitle(stringResource(R.string.details))
            // Statut (colonne)
            var colMenu by remember { mutableStateOf(false) }
            PropRow(Icons.Outlined.ViewColumn, stringResource(R.string.status)) {
                Box {
                    AssistChip(onClick = { colMenu = true }, label = { Text(board.column(task.column_id)?.name ?: "") })
                    DropdownMenu(expanded = colMenu, onDismissRequest = { colMenu = false }) {
                        board.columns.sortedBy { it.position }.forEach { c ->
                            DropdownMenuItem(text = { Text(c.name) }, onClick = { colMenu = false; if (c.id != task.column_id) state.moveTask(task, c.id) },
                                leadingIcon = { RadioButton(selected = c.id == task.column_id, onClick = null) })
                        }
                    }
                }
            }
            // Priorité
            var prioMenu by remember { mutableStateOf(false) }
            PropRow(Icons.Outlined.Flag, stringResource(R.string.priority)) {
                Box {
                    AssistChip(onClick = { prioMenu = true }, label = { Text(stringResource(PRIORITIES[task.priority.coerceIn(0, 4)])) }, leadingIcon = { PriorityIcon(task.priority) })
                    DropdownMenu(expanded = prioMenu, onDismissRequest = { prioMenu = false }) {
                        listOf(1, 2, 3, 4, 0).forEach { p ->
                            DropdownMenuItem(text = { Text(stringResource(PRIORITIES[p])) }, leadingIcon = { PriorityIcon(p) }, onClick = {
                                prioMenu = false
                                state.updateTask(task, buildJsonObject { put("priority", p) }) { it.copy(priority = p) }
                            })
                        }
                    }
                }
            }
            // Dates
            listOf("start_date" to (R.string.start_date to task.start_date), "due_date" to (R.string.due to task.due_date)).forEach { (field, info) ->
                val (label, value) = info
                PropRow(if (field == "due_date") Icons.Outlined.Event else Icons.Outlined.PlayCircleOutline, stringResource(label)) {
                    val d = localDate(value)
                    InputChip(selected = d != null, onClick = { pickDate = field }, label = { Text(d?.let { dayLabel(it) } ?: stringResource(R.string.no_due)) },
                        trailingIcon = if (d != null) { { Icon(Icons.Outlined.Close, stringResource(R.string.clear), Modifier.size(18.dp).clickable {
                            state.updateTask(task, buildJsonObject { put(field, null as String?) }) { t -> if (field == "due_date") t.copy(due_date = null) else t.copy(start_date = null) }
                        }) } } else null)
                }
            }
            // Assignés
            PropRow(Icons.Outlined.People, stringResource(R.string.assignees)) {
                Row(Modifier.clickable { assigneesSheet = true }.padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (task.assignees.isEmpty()) Text(stringResource(R.string.unassigned), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    task.assignees.forEach { a -> Avatar(state.members[a]?.name, 28.dp); Spacer(Modifier.width(4.dp)) }
                    Icon(Icons.Outlined.Edit, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            // Étiquettes
            if (board.labels.isNotEmpty()) {
                PropRow(Icons.Outlined.Label, stringResource(R.string.labels)) {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        board.labels.forEach { l ->
                            val on = task.labels.any { it.id == l.id }
                            FilterChip(selected = on, onClick = { state.setLabel(task, l, !on) }, label = { Text(l.name) },
                                leadingIcon = { Box(Modifier.size(10.dp).androidx_clipCircle(labelColor(l.color))) })
                        }
                    }
                }
            }

            // Description
            SectionTitle(stringResource(R.string.description))
            DescriptionBlock(state, board.id, task)

            // Sous-tâches
            val subtasks = board.allTasks.filter { it.parent_id == task.id }.sortedBy { it.position }
            SectionTitle(stringResource(R.string.subtasks) + if (subtasks.isNotEmpty()) " · ${subtasks.count { it.doneFor(me) }}/${subtasks.size}" else "")
            subtasks.forEach { st ->
                val sd = st.doneFor(me)
                ListItem(
                    headlineContent = { Text(st.title, textDecoration = if (sd) TextDecoration.LineThrough else null) },
                    leadingContent = { Checkbox(checked = sd, onCheckedChange = { state.toggleDone(st) }) },
                    modifier = Modifier.clickable { onTask(st.id) },
                )
            }
            var newSub by remember(task.id) { mutableStateOf("") }
            OutlinedTextField(
                value = newSub, onValueChange = { newSub = it }, singleLine = true, placeholder = { Text(stringResource(R.string.add_subtask)) },
                leadingIcon = { Icon(Icons.Outlined.Add, null) },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (newSub.isNotBlank()) { state.addSubtask(task, newSub); newSub = "" } }),
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            )

            // Pièces jointes
            val files = state.attachments[task.id].orEmpty()
            if (files.isNotEmpty()) {
                SectionTitle(stringResource(R.string.attachments))
                files.forEach { f ->
                    ListItem(headlineContent = { Text(f.filename, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        leadingContent = { Icon(Icons.Outlined.AttachFile, null) },
                        modifier = Modifier.clickable { openWeb(ctx, "/" + f.file_path.trimStart('/')) })
                }
            }

            // Commentaires
            SectionTitle(stringResource(R.string.comments))
            state.comments[task.id].orEmpty().forEach { c ->
                val who = state.members[c.user_id]?.name
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                    Avatar(who, 30.dp)
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Row { Text(who ?: "?", style = MaterialTheme.typography.labelLarge); Spacer(Modifier.width(8.dp)); Text(relativeTime(c.created_at, ctx), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                        Text(AnnotatedString.fromHtml(c.content), style = MaterialTheme.typography.bodyMedium)
                    }
                }
            }
            var comment by remember(task.id) { mutableStateOf("") }
            var sending by remember { mutableStateOf(false) }
            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(value = comment, onValueChange = { comment = it }, placeholder = { Text(stringResource(R.string.comment_placeholder)) }, modifier = Modifier.weight(1f), maxLines = 6)
                Spacer(Modifier.width(8.dp))
                FilledIconButton(enabled = comment.isNotBlank() && !sending, onClick = {
                    sending = true
                    scope.launch { if (state.addComment(task.id, comment)) comment = ""; sending = false }
                }) { Icon(Icons.AutoMirrored.Filled.Send, stringResource(R.string.send)) }
            }
        }

        // Sélecteur de date
        pickDate?.let { field ->
            val current = localDate(if (field == "due_date") task.due_date else task.start_date) ?: LocalDate.now()
            val dp = rememberDatePickerState(initialSelectedDateMillis = current.atStartOfDay().toInstant(ZoneOffset.UTC).toEpochMilli())
            DatePickerDialog(
                onDismissRequest = { pickDate = null },
                confirmButton = { TextButton(onClick = {
                    dp.selectedDateMillis?.let { ms ->
                        val day = Instant.ofEpochMilli(ms).atZone(ZoneOffset.UTC).toLocalDate()
                        val iso = day.atTime(12, 0).atZone(ZoneId.systemDefault()).toInstant().toString()
                        state.updateTask(task, buildJsonObject { put(field, iso) }) { t -> if (field == "due_date") t.copy(due_date = iso) else t.copy(start_date = iso) }
                    }
                    pickDate = null
                }) { Text("OK") } },
                dismissButton = { TextButton(onClick = { pickDate = null }) { Text(stringResource(R.string.cancel)) } },
            ) { DatePicker(dp) }
        }
        if (assigneesSheet) AssigneesSheet(state, task) { assigneesSheet = false }
    }
}

@Composable
private fun PropRow(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, content: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp).heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null, Modifier.size(20.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.width(12.dp))
        Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.width(96.dp))
        Box(Modifier.weight(1f)) { content() }
    }
}

private fun Modifier.androidx_clipCircle(c: androidx.compose.ui.graphics.Color) =
    this.then(Modifier.clip(androidx.compose.foundation.shape.CircleShape)).then(Modifier.background(c))

/** Description : affichée mise en forme ; modifiable ici si elle est simple, sinon dans l'éditeur. */
@Composable
private fun DescriptionBlock(state: AppState, boardId: String, task: Task) {
    val ctx = LocalContext.current
    var editing by remember(task.id) { mutableStateOf(false) }
    var text by remember(task.id) { mutableStateOf(Html.toPlain(task.description)) }
    val simple = Html.isSimple(task.description)
    Column(Modifier.padding(horizontal = 16.dp)) {
        if (editing) {
            // Le clavier va au champ dès l'ouverture (et pas au champ de commentaire).
            val focus = remember { androidx.compose.ui.focus.FocusRequester() }
            LaunchedEffect(Unit) { focus.requestFocus() }
            OutlinedTextField(value = text, onValueChange = { text = it }, modifier = Modifier.fillMaxWidth().focusRequester(focus), minLines = 4)
            Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.End) {
                TextButton(onClick = { editing = false; text = Html.toPlain(task.description) }) { Text(stringResource(R.string.cancel)) }
                Button(onClick = {
                    val html = Html.fromPlain(text).ifBlank { null }
                    state.updateTask(task, buildJsonObject { put("description", html) }) { it.copy(description = html) }
                    editing = false
                }) { Text(stringResource(R.string.save)) }
            }
        } else {
            val desc = task.description
            if (desc.isNullOrBlank()) Text(stringResource(R.string.no_description), color = MaterialTheme.colorScheme.onSurfaceVariant)
            else Text(AnnotatedString.fromHtml(desc), style = MaterialTheme.typography.bodyLarge)
            TextButton(onClick = {
                if (simple) editing = true else state.ws?.let { openWeb(ctx, Paths.task(it, boardId, task.id)) }
            }, contentPadding = PaddingValues(0.dp)) { Text(stringResource(if (simple) R.string.edit else R.string.edit_in_editor)) }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AssigneesSheet(state: AppState, task: Task, onDismiss: () -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.padding(bottom = 24.dp)) {
            Text(stringResource(R.string.assignees), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            state.members.values.sortedBy { it.name }.forEach { m ->
                val on = m.user_id in task.assignees
                ListItem(
                    colors = sheetItemColors(),
                    headlineContent = { Text(m.name) }, supportingContent = { Text(m.email) },
                    leadingContent = { Avatar(m.name, 32.dp) },
                    trailingContent = { Checkbox(checked = on, onCheckedChange = { state.setAssignee(task, m.user_id, !on) }) },
                    modifier = Modifier.clickable { state.setAssignee(task, m.user_id, !on) },
                )
            }
        }
    }
}
