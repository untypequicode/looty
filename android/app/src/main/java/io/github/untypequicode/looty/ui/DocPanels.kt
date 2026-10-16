package io.github.untypequicode.looty.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.DocComment
import io.github.untypequicode.looty.api.Revision
import kotlinx.coroutines.launch

/**
 * Commentaires d'un document : fils (avec le passage commenté), réponses, résolution,
 * modification et suppression de ses commentaires. Un nouveau fil porte sur le document entier
 * (les commentaires sur un passage se créent dans l'éditeur).
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun DocCommentsScreen(state: AppState, pageId: String, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<DocComment>?>(null) }
    var showResolved by remember { mutableStateOf(false) }
    var replyTo by remember { mutableStateOf<String?>(null) }
    var text by remember { mutableStateOf("") }
    var sending by remember { mutableStateOf(false) }
    var editing by remember { mutableStateOf<DocComment?>(null) }
    var deleting by remember { mutableStateOf<DocComment?>(null) }
    val focus = remember { FocusRequester() }
    val me = state.me?.id
    val page = state.pages.data?.firstOrNull { it.id == pageId }
    fun reload() { scope.launch { state.docComments(pageId)?.let { list = it } } }
    LaunchedEffect(pageId) { reload() }
    LaunchedEffect(replyTo) { if (replyTo != null) runCatching { focus.requestFocus() } }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(stringResource(R.string.comments))
                        page?.let { Text(it.title.ifBlank { stringResource(R.string.untitled) }, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                    }
                },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = { FilterChip(showResolved, { showResolved = !showResolved }, label = { Text(stringResource(R.string.show_resolved)) }, modifier = Modifier.padding(end = 8.dp)) },
            )
        },
        bottomBar = {
            if (state.canComment) Surface(tonalElevation = 3.dp) {
                Column(Modifier.navigationBarsPadding().imePadding()) {
                    replyTo?.let { id ->
                        val thread = list?.firstOrNull { it.id == id }
                        Row(Modifier.fillMaxWidth().padding(start = 16.dp, top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(stringResource(R.string.reply_to, thread?.user_name ?: "?"), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary, modifier = Modifier.weight(1f))
                            IconButton(onClick = { replyTo = null }) { Icon(Icons.Outlined.Close, stringResource(R.string.cancel), Modifier.size(18.dp)) }
                        }
                    }
                    Row(Modifier.padding(horizontal = 12.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        OutlinedTextField(text, { text = it }, placeholder = { Text(stringResource(if (replyTo != null) R.string.reply_placeholder else R.string.comment_placeholder)) },
                            modifier = Modifier.weight(1f).focusRequester(focus), maxLines = 5)
                        Spacer(Modifier.width(8.dp))
                        FilledIconButton(enabled = text.isNotBlank() && !sending, onClick = {
                            sending = true
                            scope.launch {
                                if (state.addDocComment(pageId, text, replyTo)) { text = ""; replyTo = null; reload() }
                                sending = false
                            }
                        }) { Icon(Icons.AutoMirrored.Filled.Send, stringResource(R.string.send)) }
                    }
                }
            }
        },
    ) { padding ->
        val all = list
        if (all == null) { Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }; return@Scaffold }
        val threads = all.filter { it.parent_id == null && (showResolved || it.resolved_at == null) }
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            if (threads.isEmpty()) item { EmptyState(Icons.Outlined.ChatBubbleOutline, stringResource(R.string.no_comments)) }
            items(threads, key = { it.id }) { t ->
                val replies = all.filter { it.parent_id == t.id }
                OutlinedCard(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        t.quote?.takeIf { it.isNotBlank() }?.let { q ->
                            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min)) {
                                Box(Modifier.width(3.dp).fillMaxHeight().background(MaterialTheme.colorScheme.tertiary))
                                Text(q, style = MaterialTheme.typography.bodySmall, fontStyle = FontStyle.Italic, color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 3, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(start = 8.dp))
                            }
                        }
                        (listOf(t) + replies).forEach { c ->
                            CommentItem(state, c, mine = c.user_id == me, onEdit = { editing = c }, onDelete = { deleting = c })
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            if (state.canComment) TextButton(onClick = { replyTo = t.id }) { Icon(Icons.Outlined.Reply, null, Modifier.size(18.dp)); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.reply)) }
                            TextButton(onClick = { scope.launch { if (state.resolveDocComment(t.id, t.resolved_at == null)) reload() } }) {
                                Icon(if (t.resolved_at == null) Icons.Outlined.CheckCircle else Icons.Outlined.Replay, null, Modifier.size(18.dp)); Spacer(Modifier.width(6.dp))
                                Text(stringResource(if (t.resolved_at == null) R.string.resolve else R.string.reopen))
                            }
                        }
                    }
                }
            }
        }
    }

    editing?.let { c ->
        var v by remember(c.id) { Html.toPlain(c.content).let { mutableStateOf(TextFieldValue(it, TextRange(it.length))) } }
        val f = remember { FocusRequester() }
        LaunchedEffect(c.id) { runCatching { f.requestFocus() } }
        AlertDialog(
            onDismissRequest = { editing = null },
            title = { Text(stringResource(R.string.edit)) },
            text = { OutlinedTextField(v, { v = it }, modifier = Modifier.fillMaxWidth().focusRequester(f), minLines = 3) },
            confirmButton = { TextButton(enabled = v.text.isNotBlank(), onClick = { editing = null; scope.launch { if (state.editDocComment(c.id, v.text)) reload() } }) { Text(stringResource(R.string.save)) } },
            dismissButton = { TextButton(onClick = { editing = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    deleting?.let { c ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text(stringResource(R.string.delete_comment_title)) },
            confirmButton = { TextButton(onClick = { deleting = null; scope.launch { if (state.deleteDocComment(c.id)) reload() } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun CommentItem(state: AppState, c: DocComment, mine: Boolean, onEdit: () -> Unit, onDelete: () -> Unit) {
    val ctx = LocalContext.current
    var menu by remember { mutableStateOf(false) }
    Box {
        Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).combinedClickable(onClick = {}, onLongClick = { if (mine || state.isAdmin) menu = true })) {
            Avatar(c.user_name, 28.dp)
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(c.user_name ?: "?", style = MaterialTheme.typography.labelLarge)
                    Spacer(Modifier.width(8.dp))
                    Text(relativeTime(c.created_at, ctx) + if (c.updated_at != null && c.updated_at != c.created_at) " · " + stringResource(R.string.edited) else "",
                        style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                HtmlBlocks(state, c.content)
            }
        }
        DropdownMenu(menu, { menu = false }) {
            if (mine && Html.isSimple(c.content)) DropdownMenuItem(text = { Text(stringResource(R.string.edit)) }, leadingIcon = { Icon(Icons.Outlined.Edit, null) }, onClick = { menu = false; onEdit() })
            DropdownMenuItem(text = { Text(stringResource(R.string.delete)) }, leadingIcon = { Icon(Icons.Outlined.Delete, null) }, onClick = { menu = false; onDelete() })
        }
    }
}

/**
 * Versions d'un document : une par séance d'édition, plus celles nommées. Les voir ou les
 * rétablir se fait dans l'éditeur (le contenu est celui de l'éditeur collaboratif).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DocHistoryScreen(state: AppState, pageId: String, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<Revision>?>(null) }
    var naming by remember { mutableStateOf<Revision?>(null) }
    var newVersion by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf<Revision?>(null) }
    val page = state.pages.data?.firstOrNull { it.id == pageId }
    val write = page?.let { state.canWritePage(it) } ?: false
    fun reload() { scope.launch { state.revisions(pageId)?.let { list = it } } }
    LaunchedEffect(pageId) { reload() }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.version_history)) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
        floatingActionButton = { if (write) ExtendedFloatingActionButton(onClick = { newVersion = true }, icon = { Icon(Icons.Outlined.BookmarkAdd, null) }, text = { Text(stringResource(R.string.name_version)) }) },
    ) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
            item {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.versions_hint), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    OutlinedButton(onClick = { state.ws?.let { openWeb(ctx, Paths.doc(it, pageId) + "?history=1") } }) {
                        Icon(Icons.Outlined.OpenInNew, null, Modifier.size(18.dp)); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.open_history_editor))
                    }
                }
            }
            val l = list
            if (l == null) item { Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp)) } }
            else if (l.isEmpty()) item { EmptyState(Icons.Outlined.History, stringResource(R.string.no_versions)) }
            else items(l, key = { it.id }) { r ->
                var menu by remember { mutableStateOf(false) }
                ListItem(
                    headlineContent = { Text(r.label ?: stringResource(if (r.kind == "restore") R.string.version_restore else R.string.version_auto), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = { Text(listOfNotNull(r.author_name, mailFullDate(r.updated_at ?: r.created_at)).joinToString(" · ")) },
                    leadingContent = { Icon(if (r.label != null) Icons.Outlined.Bookmark else Icons.Outlined.History, null, tint = if (r.label != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant) },
                    trailingContent = if (write) { {
                        Box {
                            IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                            DropdownMenu(menu, { menu = false }) {
                                DropdownMenuItem(text = { Text(stringResource(if (r.label != null) R.string.rename else R.string.name_version)) }, leadingIcon = { Icon(Icons.Outlined.Edit, null) }, onClick = { menu = false; naming = r })
                                DropdownMenuItem(text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) }, leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) }, onClick = { menu = false; deleting = r })
                            }
                        }
                    } } else null,
                )
            }
        }
    }
    if (newVersion) NameDialog(stringResource(R.string.name_version), stringResource(R.string.version_name), "", onDismiss = { newVersion = false }) { label ->
        newVersion = false; scope.launch { if (state.createRevision(pageId, label)) reload() }
    }
    naming?.let { r ->
        NameDialog(stringResource(R.string.name_version), stringResource(R.string.version_name), r.label.orEmpty(), onDismiss = { naming = null }) { label ->
            naming = null; scope.launch { if (state.renameRevision(pageId, r.id, label)) reload() }
        }
    }
    deleting?.let { r ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text(stringResource(R.string.delete_version_title)) },
            confirmButton = { TextButton(onClick = { deleting = null; scope.launch { if (state.deleteRevision(pageId, r.id)) reload() } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}
