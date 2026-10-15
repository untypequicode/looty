package io.github.untypequicode.looty.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.ColumnFull
import io.github.untypequicode.looty.api.Label
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

/** Paramètres d'un tableau : nom, colonnes, étiquettes, automatisations, suppression. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BoardSettingsScreen(state: AppState, boardId: String, onBack: () -> Unit, onDeleted: () -> Unit) {
    val board = state.boardData[boardId]?.data
    LaunchedEffect(boardId) { if (board == null) state.loadBoard(boardId) }
    val scope = rememberCoroutineScope()
    var renameBoard by remember { mutableStateOf(false) }
    var renameColumn by remember { mutableStateOf<ColumnFull?>(null) }
    var deleteColumn by remember { mutableStateOf<ColumnFull?>(null) }
    var addColumn by remember { mutableStateOf(false) }
    var editLabel by remember { mutableStateOf<Label?>(null) }
    var newLabel by remember { mutableStateOf(false) }
    var deleteBoard by remember { mutableStateOf(false) }

    Scaffold(topBar = {
        TopAppBar(
            title = { Text(stringResource(R.string.board_settings)) },
            navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
        )
    }) { padding ->
        if (board == null) {
            Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            return@Scaffold
        }
        val columns = board.columns.sortedBy { it.position }
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
            item {
                ListItem(
                    headlineContent = { Text(board.name) }, overlineContent = { Text(stringResource(R.string.board_name)) },
                    trailingContent = { Icon(Icons.Outlined.Edit, stringResource(R.string.rename)) },
                    modifier = Modifier.clickable { renameBoard = true },
                )
            }

            // Colonnes
            item { SectionTitle(stringResource(R.string.columns)) }
            columns.forEachIndexed { i, c ->
                item(key = "c" + c.id) {
                    var menu by remember { mutableStateOf(false) }
                    ListItem(
                        headlineContent = { Text(c.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = { Text("${c.tasks.count { it.parent_id == null }}") },
                        leadingContent = { Icon(Icons.Outlined.ViewColumn, null) },
                        trailingContent = {
                            Row {
                                IconButton(enabled = i > 0, onClick = { state.moveColumn(boardId, c.id, -1) }) { Icon(Icons.Outlined.ArrowUpward, stringResource(R.string.move_up)) }
                                IconButton(enabled = i < columns.size - 1, onClick = { state.moveColumn(boardId, c.id, 1) }) { Icon(Icons.Outlined.ArrowDownward, stringResource(R.string.move_down)) }
                                Box {
                                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                                        DropdownMenuItem(text = { Text(stringResource(R.string.rename)) }, leadingIcon = { Icon(Icons.Outlined.Edit, null) }, onClick = { menu = false; renameColumn = c })
                                        DropdownMenuItem(text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                                            leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) }, onClick = { menu = false; deleteColumn = c })
                                    }
                                }
                            }
                        },
                        modifier = Modifier.clickable { renameColumn = c },
                    )
                }
            }
            item {
                ListItem(headlineContent = { Text(stringResource(R.string.add_column), color = MaterialTheme.colorScheme.primary) },
                    leadingContent = { Icon(Icons.Filled.Add, null, tint = MaterialTheme.colorScheme.primary) }, modifier = Modifier.clickable { addColumn = true })
            }

            // Étiquettes
            item { SectionTitle(stringResource(R.string.labels)) }
            board.labels.forEach { l ->
                item(key = "l" + l.id) {
                    ListItem(
                        headlineContent = { Text(l.name) },
                        leadingContent = { Box(Modifier.size(16.dp).clip(CircleShape).background(labelColor(l.color))) },
                        trailingContent = { Icon(Icons.Outlined.Edit, null) },
                        modifier = Modifier.clickable { editLabel = l },
                    )
                }
            }
            item {
                ListItem(headlineContent = { Text(stringResource(R.string.new_label), color = MaterialTheme.colorScheme.primary) },
                    leadingContent = { Icon(Icons.Filled.Add, null, tint = MaterialTheme.colorScheme.primary) }, modifier = Modifier.clickable { newLabel = true })
            }

            // Automatisations
            item {
                SectionTitle(stringResource(R.string.automations))
                val a = board.automations
                fun bool(k: String) = a?.get(k)?.let { runCatching { it.jsonPrimitive.booleanOrNull }.getOrNull() } ?: false
                val doneCol = a?.get("on_complete_move_to")?.let { runCatching { it.jsonPrimitive.contentOrNull }.getOrNull() }?.takeIf { id -> columns.any { it.id == id } }
                val autoValidate = bool("auto_validate_on_move")
                val applySub = bool("apply_to_subtasks")
                fun save(col: String?, auto: Boolean, sub: Boolean) = state.saveAutomations(boardId, buildJsonObject {
                    put("on_complete_move_to", col?.let { JsonPrimitive(it) } ?: JsonNull)
                    put("auto_validate_on_move", auto)
                    put("apply_to_subtasks", sub)
                })
                var colMenu by remember { mutableStateOf(false) }
                Column(Modifier.padding(horizontal = 16.dp)) {
                    Text(stringResource(R.string.done_column), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Box(Modifier.padding(vertical = 8.dp)) {
                        OutlinedButton(onClick = { colMenu = true }) {
                            Text(columns.firstOrNull { it.id == doneCol }?.name ?: stringResource(R.string.done_column_none))
                            Icon(Icons.Outlined.ArrowDropDown, null)
                        }
                        DropdownMenu(expanded = colMenu, onDismissRequest = { colMenu = false }) {
                            DropdownMenuItem(text = { Text(stringResource(R.string.done_column_none)) }, leadingIcon = { RadioButton(selected = doneCol == null, onClick = null) },
                                onClick = { colMenu = false; save(null, false, false) })
                            columns.forEach { c ->
                                DropdownMenuItem(text = { Text(c.name) }, leadingIcon = { RadioButton(selected = doneCol == c.id, onClick = null) },
                                    onClick = { colMenu = false; save(c.id, autoValidate, applySub) })
                            }
                        }
                    }
                }
                ListItem(headlineContent = { Text(stringResource(R.string.auto_validate)) },
                    trailingContent = { Switch(checked = autoValidate, enabled = doneCol != null, onCheckedChange = { save(doneCol, it, applySub) }) })
                ListItem(headlineContent = { Text(stringResource(R.string.apply_subtasks)) },
                    trailingContent = { Switch(checked = applySub, enabled = doneCol != null, onCheckedChange = { save(doneCol, autoValidate, it) }) })
            }

            // Supprimer un tableau entier : réservé aux admins de l’espace (règle du serveur).
            if (state.isAdmin) item {
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                ListItem(
                    headlineContent = { Text(stringResource(R.string.delete_board), color = MaterialTheme.colorScheme.error) },
                    leadingContent = { Icon(Icons.Outlined.DeleteForever, null, tint = MaterialTheme.colorScheme.error) },
                    modifier = Modifier.clickable { deleteBoard = true },
                )
            }
        }

        if (renameBoard) NameDialog(stringResource(R.string.rename), stringResource(R.string.board_name), board.name, onDismiss = { renameBoard = false }) {
            renameBoard = false; state.renameBoard(boardId, it)
        }
        if (addColumn) NameDialog(stringResource(R.string.add_column), stringResource(R.string.column_name), "", onDismiss = { addColumn = false }) {
            addColumn = false; scope.launch { state.createColumn(boardId, it) }
        }
        renameColumn?.let { c ->
            NameDialog(stringResource(R.string.rename), stringResource(R.string.column_name), c.name, onDismiss = { renameColumn = null }) {
                renameColumn = null; state.renameColumn(boardId, c.id, it)
            }
        }
        deleteColumn?.let { c ->
            val n = c.tasks.size
            AlertDialog(
                onDismissRequest = { deleteColumn = null },
                title = { Text(stringResource(R.string.delete_column_title, c.name)) },
                text = if (n > 0) { { Text(pluralStringResource(R.plurals.delete_column_tasks, n, n)) } } else null,
                confirmButton = { TextButton(onClick = { deleteColumn = null; state.deleteColumn(boardId, c.id) }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
                dismissButton = { TextButton(onClick = { deleteColumn = null }) { Text(stringResource(R.string.cancel)) } },
            )
        }
        if (newLabel || editLabel != null) LabelDialog(editLabel, onDismiss = { newLabel = false; editLabel = null },
            onDelete = { l -> state.deleteLabel(boardId, l.id); editLabel = null },
        ) { name, color ->
            val l = editLabel
            if (l != null) state.updateLabel(boardId, l.copy(name = name, color = color)) else scope.launch { state.createLabel(boardId, name, color) }
            newLabel = false; editLabel = null
        }
        if (deleteBoard) AlertDialog(
            onDismissRequest = { deleteBoard = false },
            title = { Text(stringResource(R.string.delete_board_title, board.name)) },
            text = { Text(stringResource(R.string.delete_board_message)) },
            confirmButton = { TextButton(onClick = { deleteBoard = false; scope.launch { if (state.deleteBoard(boardId)) onDeleted() } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { deleteBoard = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** Création ou modification d'une étiquette (nom, couleur), et sa suppression. */
@Composable
private fun LabelDialog(label: Label?, onDismiss: () -> Unit, onDelete: (Label) -> Unit, onSave: (String, String) -> Unit) {
    var name by remember { mutableStateOf(label?.name.orEmpty()) }
    var color by remember { mutableStateOf(label?.color ?: LABEL_COLORS.first()) }
    var confirmDelete by remember { mutableStateOf(false) }
    if (confirmDelete && label != null) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text(stringResource(R.string.delete_label_title, label.name)) },
            text = { Text(stringResource(R.string.delete_label_message)) },
            confirmButton = { TextButton(onClick = { confirmDelete = false; onDelete(label) }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text(stringResource(R.string.cancel)) } },
        )
        return
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(label?.name ?: stringResource(R.string.new_label)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                OutlinedTextField(name, { name = it }, singleLine = true, label = { Text(stringResource(R.string.label_name)) }, modifier = Modifier.fillMaxWidth())
                Text(stringResource(R.string.label_color), style = MaterialTheme.typography.labelLarge)
                ColorPalette(color) { color = it }
            }
        },
        confirmButton = { TextButton(enabled = name.isNotBlank(), onClick = { onSave(name.trim(), color) }) { Text(stringResource(R.string.save)) } },
        dismissButton = {
            Row {
                if (label != null) TextButton(onClick = { confirmDelete = true }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) }
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
            }
        },
    )
}
