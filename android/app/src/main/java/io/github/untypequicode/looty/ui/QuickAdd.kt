package io.github.untypequicode.looty.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

/** Ajout rapide d'une tâche : titre, tableau, échéance, « m'assigner ». */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QuickAddSheet(state: AppState, onDismiss: () -> Unit, onCreated: () -> Unit) {
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    var title by remember { mutableStateOf("") }
    var boardId by remember { mutableStateOf(state.lastBoard?.takeIf { id -> state.boards.any { it.id == id } } ?: state.boards.firstOrNull()?.id) }
    var due by remember { mutableStateOf<LocalDate?>(null) }
    var assignMe by remember { mutableStateOf(true) }
    var busy by remember { mutableStateOf(false) }
    var pickDate by remember { mutableStateOf(false) }
    var boardMenu by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }

    fun submit() {
        val b = boardId ?: return
        if (title.isBlank() || busy) return
        busy = true
        scope.launch {
            state.createTask(b, title, due, assignMe).onSuccess { onCreated() }
            busy = false
        }
    }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheet) {
        Column(Modifier.padding(horizontal = 20.dp).padding(bottom = 24.dp).imePadding(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Text(stringResource(R.string.new_task), style = MaterialTheme.typography.titleLarge)
            OutlinedTextField(
                value = title, onValueChange = { title = it }, label = { Text(stringResource(R.string.task_title)) },
                singleLine = true, modifier = Modifier.fillMaxWidth().focusRequester(focus),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done), keyboardActions = KeyboardActions(onDone = { submit() }),
            )
            ExposedDropdownMenuBox(expanded = boardMenu, onExpandedChange = { boardMenu = it }) {
                OutlinedTextField(
                    value = state.boards.firstOrNull { it.id == boardId }?.name ?: "", onValueChange = {}, readOnly = true,
                    label = { Text(stringResource(R.string.board)) }, trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(boardMenu) },
                    modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
                )
                ExposedDropdownMenu(expanded = boardMenu, onDismissRequest = { boardMenu = false }) {
                    state.boards.forEach { b -> DropdownMenuItem(text = { Text(b.name) }, onClick = { boardId = b.id; boardMenu = false }) }
                }
            }
            Text(stringResource(R.string.due), style = MaterialTheme.typography.labelLarge)
            Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                val today = LocalDate.now()
                listOf(null to R.string.no_due, today to R.string.today, today.plusDays(1) to R.string.tomorrow, today.plusDays(7) to R.string.next_week).forEach { (d, label) ->
                    FilterChip(selected = due == d, onClick = { due = d }, label = { Text(stringResource(label)) })
                }
                val custom = due?.takeIf { it != today && it != today.plusDays(1) && it != today.plusDays(7) }
                FilterChip(selected = custom != null, onClick = { pickDate = true }, label = { Text(custom?.let { dayLabel(it) } ?: stringResource(R.string.pick_date)) })
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Switch(checked = assignMe, onCheckedChange = { assignMe = it })
                Spacer(Modifier.width(12.dp))
                Text(stringResource(R.string.assign_me))
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
                Spacer(Modifier.width(8.dp))
                Button(onClick = ::submit, enabled = title.isNotBlank() && boardId != null && !busy) { Text(stringResource(R.string.create)) }
            }
        }
    }
    if (pickDate) {
        val dp = rememberDatePickerState(initialSelectedDateMillis = (due ?: LocalDate.now()).atStartOfDay().toInstant(ZoneOffset.UTC).toEpochMilli())
        DatePickerDialog(
            onDismissRequest = { pickDate = false },
            confirmButton = { TextButton(onClick = { dp.selectedDateMillis?.let { due = Instant.ofEpochMilli(it).atZone(ZoneOffset.UTC).toLocalDate() }; pickDate = false }) { Text("OK") } },
            dismissButton = { TextButton(onClick = { pickDate = false }) { Text(stringResource(R.string.cancel)) } },
        ) { DatePicker(dp) }
    }
}
