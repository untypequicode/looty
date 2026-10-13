package io.github.untypequicode.looty

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import io.github.untypequicode.looty.ui.Html
import io.github.untypequicode.looty.ui.LoadingIndicatorCompat
import io.github.untypequicode.looty.ui.LootyTheme
import io.github.untypequicode.looty.ui.NoNavScrim
import io.github.untypequicode.looty.ui.Paths
import kotlinx.coroutines.launch

/**
 * « Partager » vers Looty : le texte ou le lien reçu d'une autre application devient une
 * tâche (dans le tableau choisi) ou un document. Écran translucide : une feuille au-dessus
 * de l'application d'où vient le partage.
 */
class ShareActivity : ComponentActivity() {
    private val state: AppState by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val text = intent.getStringExtra(Intent.EXTRA_TEXT)?.trim().orEmpty()
        val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT)?.trim().orEmpty()
        if (Config.server(this) == null || text.isEmpty() && subject.isEmpty()) {
            Toast.makeText(this, R.string.share_nothing, Toast.LENGTH_SHORT).show()
            finish()
            return
        }
        if (savedInstanceState == null) state.start()
        setContent {
            LootyTheme {
                ShareSheet(state, subject, text, onClose = { finish() })
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ShareSheet(state: AppState, subject: String, text: String, onClose: () -> Unit) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val scope = rememberCoroutineScope()
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    // Titre proposé : l'objet du partage, sinon la première ligne du texte.
    val suggested = remember { (subject.ifEmpty { text.lineSequence().firstOrNull { it.isNotBlank() }.orEmpty() }).take(120) }
    var title by remember { mutableStateOf(suggested) }
    var asDoc by remember { mutableStateOf(false) }
    var boardId by remember(state.boards) { mutableStateOf(state.lastBoard?.takeIf { id -> state.boards.any { it.id == id } } ?: state.boards.firstOrNull()?.id) }
    var boardMenu by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    var failed by remember { mutableStateOf(false) }
    val focus = remember { FocusRequester() }
    val ready = state.session is Session.Ready
    val saved = stringResource(R.string.share_saved)

    fun submit() {
        if (title.isBlank() || busy) return
        busy = true; failed = false
        scope.launch {
            // Le texte complet va dans la description / le corps, sauf s'il se résume au titre.
            val body = text.takeIf { it.isNotBlank() && it != title.trim() }?.let { Html.fromPlain(it) }
            val result = if (asDoc) state.createPage(title.trim(), Html.fromPlain(text.ifBlank { subject }))
            else boardId?.let { b -> state.createTask(b, title, null, assignMe = true, description = body) } ?: Result.failure(IllegalStateException("no_board"))
            busy = false
            result.onSuccess {
                android.widget.Toast.makeText(ctx, saved, android.widget.Toast.LENGTH_SHORT).show()
                onClose()
            }.onFailure { failed = true }
        }
    }

    ModalBottomSheet(onDismissRequest = onClose, sheetState = sheet) {
        NoNavScrim()
        Column(Modifier.padding(horizontal = 20.dp).padding(bottom = 24.dp).imePadding().verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Text(stringResource(R.string.share_title), style = MaterialTheme.typography.titleLarge)
            when (state.session) {
                is Session.NeedLogin, is Session.NeedServer -> {
                    Text(stringResource(R.string.share_sign_in), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Button(onClick = { ctx.startActivity(Intent(ctx, MainActivity::class.java)); onClose() }) { Text(stringResource(R.string.sign_in)) }
                }
                is Session.Offline -> {
                    Text(stringResource(R.string.offline_hint), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Button(onClick = { state.start() }) { Text(stringResource(R.string.retry)) }
                }
                is Session.Loading -> Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                is Session.Ready -> {
                    val canTask = state.canEditBoards
                    val canDoc = state.canEditDocs
                    LaunchedEffect(canTask, canDoc) { if (!canTask && canDoc) asDoc = true }
                    if (!canTask && !canDoc) Text(stringResource(R.string.share_no_rights), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    else {
                        Text(text.ifEmpty { subject }, maxLines = 3, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        if (canTask && canDoc) SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                            SegmentedButton(selected = !asDoc, onClick = { asDoc = false }, shape = SegmentedButtonDefaults.itemShape(0, 2)) { Text(stringResource(R.string.share_as_task)) }
                            SegmentedButton(selected = asDoc, onClick = { asDoc = true }, shape = SegmentedButtonDefaults.itemShape(1, 2)) { Text(stringResource(R.string.share_as_doc)) }
                        }
                        OutlinedTextField(
                            value = title, onValueChange = { title = it }, label = { Text(stringResource(if (asDoc) R.string.doc_title else R.string.task_title)) },
                            singleLine = true, modifier = Modifier.fillMaxWidth().focusRequester(focus),
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done), keyboardActions = KeyboardActions(onDone = { submit() }),
                        )
                        if (!asDoc) ExposedDropdownMenuBox(expanded = boardMenu, onExpandedChange = { boardMenu = it }) {
                            OutlinedTextField(
                                value = state.boards.firstOrNull { it.id == boardId }?.name ?: "", onValueChange = {}, readOnly = true,
                                label = { Text(stringResource(R.string.board)) }, trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(boardMenu) },
                                modifier = Modifier.fillMaxWidth().menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
                            )
                            ExposedDropdownMenu(expanded = boardMenu, onDismissRequest = { boardMenu = false }) {
                                state.boards.forEach { b -> DropdownMenuItem(text = { Text(b.name) }, onClick = { boardId = b.id; boardMenu = false }) }
                            }
                        }
                        if (failed) Text(stringResource(R.string.share_failed), color = MaterialTheme.colorScheme.error)
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
                            TextButton(onClick = onClose) { Text(stringResource(R.string.cancel)) }
                            Spacer(Modifier.width(8.dp))
                            Button(onClick = ::submit, enabled = title.isNotBlank() && !busy && (asDoc || boardId != null)) { Text(stringResource(R.string.create)) }
                        }
                    }
                }
            }
        }
    }
}
