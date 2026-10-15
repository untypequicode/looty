package io.github.untypequicode.looty.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import android.webkit.MimeTypeMap
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Attachment
import io.github.untypequicode.looty.api.BoardFull
import io.github.untypequicode.looty.api.Comment
import io.github.untypequicode.looty.api.Label
import io.github.untypequicode.looty.api.Task
import io.github.untypequicode.looty.api.TaskActivity
import io.github.untypequicode.looty.readSharedFile
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset

private val PRIORITIES = listOf(R.string.prio_0, R.string.prio_1, R.string.prio_2, R.string.prio_3, R.string.prio_4)

/** Couleurs proposées pour les étiquettes (les mêmes que sur le site). */
val LABEL_COLORS = listOf("#7170ff", "#4ea7fc", "#26b5ce", "#4cb782", "#f2c94c", "#f2994a", "#eb5757", "#e06ba8", "#b18cff", "#8a8f98")

/** Détail d'une tâche, entièrement natif. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun TaskScreen(state: AppState, taskId: String, onBack: () -> Unit, onTask: (String) -> Unit) {
    val ctx = LocalContext.current
    val haptics = LocalHapticFeedback.current
    LaunchedEffect(taskId) { state.ensureTask(taskId); state.loadComments(taskId); state.loadActivities(taskId) }
    val board = state.boardOf(taskId)
    val task = board?.task(taskId)
    val me = state.me?.id
    // Droits du rôle : modifier la tâche, la terminer, commenter.
    val edit = state.canEditBoards
    val validate = state.canValidate
    var menu by remember { mutableStateOf(false) }
    var pickDate by remember { mutableStateOf<String?>(null) }
    var assigneesSheet by remember { mutableStateOf(false) }
    var labelsSheet by remember { mutableStateOf(false) }
    val editor = task?.let { t -> rememberDescEditor(t.id, t.description) { html -> state.updateTask(t, buildJsonObject { put("description", html) }) { it.copy(description = html) } } }

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
                        if (task != null && edit) DropdownMenuItem(text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                            leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) },
                            onClick = { menu = false; state.deleteTask(task); onBack() })
                    }
                },
            )
        },
        bottomBar = { editor?.let { DescEditorToolbar(it) } },
    ) { padding ->
        if (task == null || board == null || editor == null) {
            Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            return@Scaffold
        }
        val done = task.doneFor(me)
        Column(Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).imePadding().padding(bottom = 32.dp)) {
            // Tâche parente (sous-tâche)
            task.parent_id?.let { pid -> board.task(pid) }?.let { parent ->
                AssistChip(onClick = { onTask(parent.id) }, label = { Text(parent.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    leadingIcon = { Icon(Icons.Outlined.SubdirectoryArrowLeft, null, Modifier.size(18.dp)) },
                    modifier = Modifier.padding(horizontal = 16.dp).widthIn(max = 320.dp))
            }
            // Titre (enregistré en quittant le champ)
            var title by remember(task.id) { mutableStateOf(task.title) }
            LaunchedEffect(task.title) { title = task.title }
            fun saveTitle() { if (title.isNotBlank() && title.trim() != task.title) state.updateTask(task, buildJsonObject { put("title", title.trim()) }) { it.copy(title = title.trim()) } }
            TextField(
                value = title, onValueChange = { title = it.replace("\n", " ") }, readOnly = !edit, textStyle = MaterialTheme.typography.headlineSmall,
                colors = TextFieldDefaults.colors(focusedContainerColor = MaterialTheme.colorScheme.surface, unfocusedContainerColor = MaterialTheme.colorScheme.surface,
                    focusedIndicatorColor = MaterialTheme.colorScheme.primary, unfocusedIndicatorColor = MaterialTheme.colorScheme.surface),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { saveTitle() }),
                modifier = Modifier.fillMaxWidth().padding(horizontal = 4.dp).onFocusChanged { f -> if (!f.isFocused) saveTitle() },
            )
            // Terminer / rouvrir
            Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                if (done) FilledTonalButton(enabled = validate, onClick = { haptics.performHapticFeedback(HapticFeedbackType.Confirm); state.toggleDone(task) }) {
                    Icon(Icons.Filled.CheckCircle, null); Spacer(Modifier.width(8.dp)); Text(stringResource(if (task.is_per_person) R.string.done_by_me else R.string.completed))
                } else Button(enabled = validate, onClick = { haptics.performHapticFeedback(HapticFeedbackType.Confirm); state.toggleDone(task) }) {
                    Icon(Icons.Outlined.TaskAlt, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.mark_done))
                }
                if (task.is_per_person && task.assignees.isNotEmpty()) {
                    Spacer(Modifier.width(12.dp))
                    Text(stringResource(R.string.validations, task.validations.count { it in task.assignees }, task.assignees.size), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }

            // Description, modifiable sur place
            Box(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                if (edit) DescriptionEditor(state, editor, Modifier.fillMaxWidth())
                else if (task.description.isNullOrBlank()) Text(stringResource(R.string.no_description), color = MaterialTheme.colorScheme.onSurfaceVariant)
                else HtmlBlocks(state, task.description)
            }

            SectionTitle(stringResource(R.string.details))
            // Statut (colonne)
            var colMenu by remember { mutableStateOf(false) }
            PropRow(Icons.Outlined.ViewColumn, stringResource(R.string.status)) {
                Box {
                    AssistChip(enabled = edit, onClick = { colMenu = true }, label = { Text(board.column(task.column_id)?.name ?: "") })
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
                    AssistChip(enabled = edit, onClick = { prioMenu = true }, label = { Text(stringResource(PRIORITIES[task.priority.coerceIn(0, 4)])) }, leadingIcon = { PriorityIcon(task.priority) })
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
                    InputChip(enabled = edit, selected = d != null, onClick = { pickDate = field }, label = { Text(d?.let { dayLabel(it) } ?: stringResource(R.string.no_due)) },
                        trailingIcon = if (d != null && edit) { { Icon(Icons.Outlined.Close, stringResource(R.string.clear), Modifier.size(18.dp).clickable {
                            state.updateTask(task, buildJsonObject { put(field, null as String?) }) { t -> if (field == "due_date") t.copy(due_date = null) else t.copy(start_date = null) }
                        }) } } else null)
                }
            }
            // Assignés
            PropRow(Icons.Outlined.People, stringResource(R.string.assignees)) {
                Row(Modifier.clip(MaterialTheme.shapes.small).clickable(enabled = edit) { assigneesSheet = true }.padding(4.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (task.assignees.isEmpty()) Text(stringResource(R.string.unassigned), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    task.assignees.forEach { a -> Avatar(state.members[a]?.name, 28.dp); Spacer(Modifier.width(4.dp)) }
                    if (edit) Icon(Icons.Outlined.Edit, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            // Étiquettes
            PropRow(Icons.Outlined.Label, stringResource(R.string.labels)) {
                FlowRow(Modifier.clip(MaterialTheme.shapes.small).clickable(enabled = edit) { labelsSheet = true }.padding(4.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp), itemVerticalAlignment = Alignment.CenterVertically) {
                    if (task.labels.isEmpty()) Text(if (edit) stringResource(R.string.add_labels) else "—", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    task.labels.forEach { l -> LabelPill(l) }
                }
            }
            // Validation par personne
            PropRow(Icons.Outlined.Groups, stringResource(R.string.per_person)) {
                Switch(checked = task.is_per_person, enabled = edit, onCheckedChange = { on -> state.updateTask(task, buildJsonObject { put("is_per_person", on) }) { it.copy(is_per_person = on) } })
            }
            if (task.is_per_person) {
                PropRow(Icons.Outlined.DoneAll, stringResource(R.string.require_all)) {
                    Switch(checked = task.require_all_validations, enabled = edit, onCheckedChange = { on -> state.updateTask(task, buildJsonObject { put("require_all_validations", on) }) { it.copy(require_all_validations = on) } })
                }
                if (task.assignees.isNotEmpty()) FlowRow(Modifier.padding(start = 48.dp, end = 16.dp, bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    task.assignees.forEach { a ->
                        val ok = a in task.validations
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Avatar(state.members[a]?.name, 22.dp)
                            Spacer(Modifier.width(4.dp))
                            Text(state.members[a]?.name ?: "?", style = MaterialTheme.typography.bodySmall)
                            Icon(if (ok) Icons.Filled.CheckCircle else Icons.Outlined.RadioButtonUnchecked, null, Modifier.padding(start = 2.dp).size(16.dp),
                                tint = if (ok) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline)
                        }
                    }
                }
            } else Text(stringResource(R.string.per_person_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 48.dp, end = 16.dp))

            // Sous-tâches
            val subtasks = board.allTasks.filter { it.parent_id == task.id }.sortedBy { it.position }
            SectionTitle(stringResource(R.string.subtasks) + if (subtasks.isNotEmpty()) " · ${subtasks.count { it.doneFor(me) }}/${subtasks.size}" else "")
            subtasks.forEach { st ->
                val sd = st.doneFor(me)
                ListItem(
                    headlineContent = { Text(st.title, textDecoration = if (sd) TextDecoration.LineThrough else null) },
                    leadingContent = { Checkbox(checked = sd, enabled = validate, onCheckedChange = { state.toggleDone(st) }) },
                    trailingContent = { Icon(Icons.Outlined.ChevronRight, null) },
                    modifier = Modifier.clickable { onTask(st.id) },
                )
            }
            var newSub by remember(task.id) { mutableStateOf("") }
            if (edit) OutlinedTextField(
                value = newSub, onValueChange = { newSub = it }, singleLine = true, placeholder = { Text(stringResource(R.string.add_subtask)) },
                leadingIcon = { Icon(Icons.Outlined.Add, null) },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (newSub.isNotBlank()) { state.addSubtask(task, newSub); newSub = "" } }),
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            )

            AttachmentsSection(state, task, edit)
            ActivitySection(state, board, task)
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
        if (labelsSheet) LabelsSheet(state, board, task) { labelsSheet = false }
    }
}

@Composable
private fun PropRow(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, content: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp).heightIn(min = 48.dp), verticalAlignment = Alignment.CenterVertically) {
        Icon(icon, null, Modifier.size(20.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.width(12.dp))
        Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.width(112.dp))
        Box(Modifier.weight(1f)) { content() }
    }
}

/** Étiquette : pastille de couleur et nom. */
@Composable
fun LabelPill(l: Label) {
    Row(Modifier.clip(MaterialTheme.shapes.small).background(labelColor(l.color).copy(alpha = 0.16f)).padding(horizontal = 8.dp, vertical = 3.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(labelColor(l.color)))
        Spacer(Modifier.width(5.dp))
        Text(l.name, style = MaterialTheme.typography.labelMedium, maxLines = 1)
    }
}

/** Choix de couleur parmi celles du site. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ColorPalette(selected: String, onPick: (String) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        LABEL_COLORS.forEach { c ->
            val on = c.equals(selected, ignoreCase = true)
            Box(Modifier.size(32.dp).clip(CircleShape).background(labelColor(c)).clickable { onPick(c) }, contentAlignment = Alignment.Center) {
                if (on) Icon(Icons.Filled.CheckCircle, null, tint = Color.White, modifier = Modifier.size(18.dp))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AssigneesSheet(state: AppState, task: Task, onDismiss: () -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 24.dp)) {
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

/** Étiquettes de la tâche ; création d'une étiquette pour qui gère le tableau. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LabelsSheet(state: AppState, board: BoardFull, task: Task, onDismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().imePadding().padding(bottom = 24.dp)) {
            Text(stringResource(R.string.labels), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            board.labels.forEach { l ->
                val on = task.labels.any { it.id == l.id }
                ListItem(
                    colors = sheetItemColors(),
                    headlineContent = { Text(l.name) },
                    leadingContent = { Box(Modifier.size(14.dp).clip(CircleShape).background(labelColor(l.color))) },
                    trailingContent = { Checkbox(checked = on, onCheckedChange = { state.setLabel(task, l, !on) }) },
                    modifier = Modifier.clickable { state.setLabel(task, l, !on) },
                )
            }
            if (state.canManageBoards) {
                var name by remember { mutableStateOf("") }
                var color by remember { mutableStateOf(LABEL_COLORS.first()) }
                var busy by remember { mutableStateOf(false) }
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                Text(stringResource(R.string.new_label), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(horizontal = 24.dp, vertical = 4.dp))
                Column(Modifier.padding(horizontal = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    val create = {
                        if (name.isNotBlank() && !busy) {
                            busy = true
                            scope.launch {
                                state.createLabel(board.id, name, color)?.let { l -> state.setLabel(task, l, true) }
                                name = ""; busy = false
                            }
                        }
                    }
                    OutlinedTextField(name, { name = it }, singleLine = true, label = { Text(stringResource(R.string.label_name)) }, modifier = Modifier.fillMaxWidth(),
                        leadingIcon = { Box(Modifier.size(14.dp).clip(CircleShape).background(labelColor(color))) },
                        trailingIcon = { IconButton(enabled = name.isNotBlank() && !busy, onClick = { create() }) { Icon(Icons.Outlined.Add, stringResource(R.string.add)) } },
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done), keyboardActions = KeyboardActions(onDone = { create() }))
                    ColorPalette(color) { color = it }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Pièces jointes
// ---------------------------------------------------------------------------

@Composable
private fun AttachmentsSection(state: AppState, task: Task, edit: Boolean) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val files = state.attachments[task.id].orEmpty()
    var sending by remember { mutableIntStateOf(0) }
    var opening by remember { mutableStateOf<String?>(null) }
    var confirm by remember { mutableStateOf<Attachment?>(null) }
    val res = ctx.resources
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        sending = uris.size
        scope.launch {
            val picked = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) { uris.mapNotNull { readSharedFile(ctx, it) } }
            val failed = (uris.size - picked.size) + state.addAttachments(task.id, picked)
            sending = 0
            if (failed > 0) Toast.makeText(ctx, res.getQuantityString(R.plurals.upload_failed, failed, failed), Toast.LENGTH_LONG).show()
        }
    }
    if (files.isEmpty() && !edit) return
    SectionTitle(stringResource(R.string.attachments), action = if (edit) { {
        TextButton(enabled = sending == 0, onClick = { pick.launch(arrayOf("*/*")) }) {
            Icon(Icons.Outlined.AttachFile, null, Modifier.size(18.dp)); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.add_file))
        }
    } } else null)
    if (sending > 0) LinearProgressIndicator(Modifier.fillMaxWidth().padding(horizontal = 16.dp))
    files.forEach { f ->
        var menu by remember { mutableStateOf(false) }
        ListItem(
            headlineContent = { Text(f.filename, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            supportingContent = f.size_bytes?.let { { Text(android.text.format.Formatter.formatShortFileSize(ctx, it)) } },
            leadingContent = {
                if (opening == f.id) CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                else Icon(fileIcon(f.mime_type), null)
            },
            trailingContent = if (edit) { {
                Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.delete)) }, leadingIcon = { Icon(Icons.Outlined.Delete, null) }, onClick = { menu = false; confirm = f })
                    }
                }
            } } else null,
            modifier = Modifier.clickable(enabled = opening == null) {
                opening = f.id
                scope.launch {
                    state.attachmentFile(f)?.let { file -> openFile(ctx, file, f.mime_type) }
                    opening = null
                }
            },
        )
    }
    confirm?.let { f ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text(stringResource(R.string.delete_attachment_title, f.filename)) },
            confirmButton = { TextButton(onClick = { confirm = null; state.deleteAttachment(task.id, f) }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

private fun fileIcon(mime: String?) = when {
    mime == null -> Icons.Outlined.InsertDriveFile
    mime.startsWith("image/") -> Icons.Outlined.Image
    mime == "application/pdf" -> Icons.Outlined.PictureAsPdf
    mime.startsWith("video/") -> Icons.Outlined.Movie
    mime.startsWith("audio/") -> Icons.Outlined.AudioFile
    mime.startsWith("text/") -> Icons.Outlined.Description
    else -> Icons.Outlined.InsertDriveFile
}

/** Ouvre un fichier téléchargé avec l'application choisie par le téléphone. */
private fun openFile(ctx: android.content.Context, file: java.io.File, mime: String?) {
    val uri = FileProvider.getUriForFile(ctx, ctx.packageName + ".files", file)
    val type = mime?.takeIf { it.isNotBlank() } ?: MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase()) ?: "application/octet-stream"
    val intent = Intent(Intent.ACTION_VIEW).setDataAndType(uri, type).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    try { ctx.startActivity(intent) } catch (_: ActivityNotFoundException) { Toast.makeText(ctx, R.string.no_app_for_file, Toast.LENGTH_LONG).show() }
}

// ---------------------------------------------------------------------------
// Commentaires et historique
// ---------------------------------------------------------------------------

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ActivitySection(state: AppState, board: BoardFull, task: Task) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val me = state.me?.id
    var history by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<Comment?>(null) }
    var deleting by remember { mutableStateOf<Comment?>(null) }
    var restoring by remember { mutableStateOf<TaskActivity?>(null) }
    LaunchedEffect(task.updated_at) { state.loadActivities(task.id) }

    SectionTitle(stringResource(R.string.comments), action = {
        FilterChip(selected = history, onClick = { history = !history }, label = { Text(stringResource(R.string.show_history)) },
            leadingIcon = { Icon(Icons.Outlined.History, null, Modifier.size(18.dp)) })
    })
    val comments = state.comments[task.id].orEmpty()
    val entries: List<Pair<String, Any>> = (comments.map { it.created_at to it } +
        (if (history) state.activities[task.id].orEmpty().map { it.created_at to it } else emptyList())).sortedBy { it.first }
    entries.forEach { (_, e) ->
        when (e) {
            is Comment -> {
                val who = state.members[e.user_id]?.name
                val mine = e.user_id == me
                var menu by remember { mutableStateOf(false) }
                Box {
                    Row(Modifier.fillMaxWidth().combinedClickable(onClick = {}, onLongClick = { if (mine || state.isAdmin) menu = true }).padding(horizontal = 16.dp, vertical = 8.dp)) {
                        Avatar(who, 30.dp)
                        Spacer(Modifier.width(10.dp))
                        Column(Modifier.weight(1f)) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(who ?: "?", style = MaterialTheme.typography.labelLarge); Spacer(Modifier.width(8.dp))
                                Text(relativeTime(e.created_at, ctx) + if (e.updated_at != null && e.updated_at != e.created_at) " · " + stringResource(R.string.edited) else "",
                                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            HtmlBlocks(state, e.content)
                        }
                    }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        if (mine && Html.isSimple(e.content)) DropdownMenuItem(text = { Text(stringResource(R.string.edit)) }, leadingIcon = { Icon(Icons.Outlined.Edit, null) }, onClick = { menu = false; editing = e })
                        DropdownMenuItem(text = { Text(stringResource(R.string.delete)) }, leadingIcon = { Icon(Icons.Outlined.Delete, null) }, onClick = { menu = false; deleting = e })
                    }
                }
            }
            is TaskActivity -> {
                val restorable = state.canEditBoards && e.activity_type in setOf("update_title", "update_description")
                Row(Modifier.fillMaxWidth().clickable(enabled = restorable) { restoring = e }.padding(horizontal = 16.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.width(30.dp), contentAlignment = Alignment.Center) { Icon(activityIcon(e.activity_type), null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant) }
                    Spacer(Modifier.width(10.dp))
                    Text(
                        (state.members[e.user_id]?.name ?: "?") + " " + describe(e, state, board) + " · " + relativeTime(e.created_at, ctx),
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.weight(1f),
                    )
                    if (restorable) Icon(Icons.Outlined.Restore, stringResource(R.string.history_restore), Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
                }
            }
        }
    }

    var comment by remember(task.id) { mutableStateOf("") }
    var sending by remember { mutableStateOf(false) }
    if (state.canComment) Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(value = comment, onValueChange = { comment = it }, placeholder = { Text(stringResource(R.string.comment_placeholder)) }, modifier = Modifier.weight(1f), maxLines = 6)
        Spacer(Modifier.width(8.dp))
        FilledIconButton(enabled = comment.isNotBlank() && !sending, onClick = {
            sending = true
            scope.launch { if (state.addComment(task.id, comment)) comment = ""; sending = false }
        }) { Icon(Icons.AutoMirrored.Filled.Send, stringResource(R.string.send)) }
    }

    editing?.let { c ->
        // Le clavier s'ouvre avec le curseur à la fin du commentaire.
        var text by remember(c.id) { Html.toPlain(c.content).let { mutableStateOf(TextFieldValue(it, TextRange(it.length))) } }
        val focus = remember { FocusRequester() }
        LaunchedEffect(c.id) { focus.requestFocus() }
        AlertDialog(
            onDismissRequest = { editing = null },
            title = { Text(stringResource(R.string.edit)) },
            text = { OutlinedTextField(text, { text = it }, modifier = Modifier.fillMaxWidth().focusRequester(focus), minLines = 3) },
            confirmButton = { TextButton(enabled = text.text.isNotBlank(), onClick = { editing = null; state.editComment(task.id, c, text.text) }) { Text(stringResource(R.string.save)) } },
            dismissButton = { TextButton(onClick = { editing = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    deleting?.let { c ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text(stringResource(R.string.delete_comment_title)) },
            confirmButton = { TextButton(onClick = { deleting = null; state.deleteComment(task.id, c) }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    restoring?.let { a ->
        AlertDialog(
            onDismissRequest = { restoring = null },
            title = { Text(stringResource(R.string.history_restore)) },
            text = { Text(stringResource(R.string.history_restore_message)) },
            confirmButton = { TextButton(onClick = { restoring = null; state.restoreActivity(task.id, a) }) { Text(stringResource(R.string.history_restore)) } },
            dismissButton = { TextButton(onClick = { restoring = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

private fun activityIcon(type: String) = when (type) {
    "create" -> Icons.Outlined.AddCircleOutline
    "move_column" -> Icons.Outlined.ArrowForward
    "toggle_validation" -> Icons.Outlined.TaskAlt
    "add_assignee", "remove_assignee" -> Icons.Outlined.Person
    "add_label", "remove_label" -> Icons.Outlined.Label
    "upload_attachment" -> Icons.Outlined.AttachFile
    "update_title" -> Icons.Outlined.Title
    "update_description" -> Icons.Outlined.Notes
    "update_priority" -> Icons.Outlined.Flag
    "update_due_date" -> Icons.Outlined.Event
    else -> Icons.Outlined.Edit
}

@Composable
private fun describe(a: TaskActivity, state: AppState, board: BoardFull): String {
    fun JsonObject?.str(k: String) = this?.get(k)?.let { runCatching { it.jsonPrimitive.contentOrNull }.getOrNull() }
    val nv = a.new_value as? JsonObject
    val ov = a.old_value as? JsonObject
    fun col(id: String?) = board.columns.firstOrNull { it.id == id }?.name ?: "?"
    fun label(id: String?) = board.labels.firstOrNull { it.id == id }?.name ?: "?"
    fun member(id: String?) = state.members[id]?.name ?: "?"
    return when (a.activity_type) {
        "create" -> stringResource(R.string.history_created)
        "move_column" -> stringResource(R.string.history_moved, col(ov.str("column_id")), col(nv.str("column_id")))
        "toggle_validation" -> stringResource(if (nv.str("is_completed") == "true") R.string.history_completed else R.string.history_reopened)
        "add_assignee" -> stringResource(R.string.history_assigned, member(nv.str("user_id")))
        "remove_assignee" -> stringResource(R.string.history_unassigned, member(ov.str("user_id")))
        "add_label" -> stringResource(R.string.history_label_added, label(nv.str("label_id")))
        "remove_label" -> stringResource(R.string.history_label_removed, label(ov.str("label_id")))
        "upload_attachment" -> stringResource(R.string.history_attached, nv.str("filename").orEmpty())
        "update_title" -> stringResource(R.string.history_renamed, nv.str("title").orEmpty())
        "update_description" -> stringResource(R.string.history_description)
        "update_priority" -> stringResource(R.string.history_priority, stringResource(PRIORITIES[(nv?.get("priority")?.let { runCatching { it.jsonPrimitive.intOrNull }.getOrNull() } ?: 0).coerceIn(0, 4)]))
        "update_due_date" -> nv.str("due_date")?.let { localDate(it) }?.let { stringResource(R.string.history_due, dayLabel(it)) } ?: stringResource(R.string.history_due_removed)
        else -> stringResource(R.string.history_updated)
    }
}
