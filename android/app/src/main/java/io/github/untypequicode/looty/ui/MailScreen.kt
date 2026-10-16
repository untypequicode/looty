package io.github.untypequicode.looty.ui

import android.annotation.SuppressLint
import android.content.Intent
import android.net.Uri
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.Reply
import androidx.compose.material.icons.automirrored.outlined.ReplyAll
import androidx.compose.material.icons.automirrored.outlined.Send
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.MailAddress
import io.github.untypequicode.looty.api.MailFolder
import io.github.untypequicode.looty.api.MailMessage
import io.github.untypequicode.looty.api.MailSummary
import androidx.compose.ui.text.withLink
import kotlinx.coroutines.launch
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

private fun mailTime(iso: String?): String {
    val t = runCatching { Instant.parse(iso) }.getOrNull()?.atZone(ZoneId.systemDefault()) ?: return ""
    return when {
        t.toLocalDate() == LocalDate.now() -> t.format(DateTimeFormatter.ofPattern("HH:mm"))
        t.year == LocalDate.now().year -> t.format(DateTimeFormatter.ofPattern("d MMM"))
        else -> t.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT))
    }
}

/** Date complète (citation d'une réponse, en-tête d'un transfert). */
fun mailFullDate(iso: String?): String =
    runCatching { Instant.parse(iso).atZone(ZoneId.systemDefault()).format(DateTimeFormatter.ofLocalizedDateTime(FormatStyle.MEDIUM, FormatStyle.SHORT)) }.getOrDefault("")

private fun openUrl(ctx: android.content.Context, url: String) {
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

private val ROLE_LABELS = mapOf("inbox" to R.string.folder_inbox, "sent" to R.string.folder_sent, "drafts" to R.string.folder_drafts,
    "archive" to R.string.folder_archive, "junk" to R.string.folder_junk, "trash" to R.string.folder_trash)

/** Nom d'un dossier : dossiers spéciaux traduits (comme sur le site), les autres tels quels. */
@Composable
fun folderLabel(f: MailFolder?, name: String = f?.name ?: "INBOX"): String {
    val role = f?.role ?: if (name.equals("INBOX", true)) "inbox" else null
    val res = ROLE_LABELS[role]
    return if (res != null && (role == "inbox" || f?.parent == null)) stringResource(res) else f?.label?.ifBlank { null } ?: name
}

fun folderIcon(role: String?): ImageVector = when (role) {
    "inbox" -> Icons.Outlined.Inbox
    "sent" -> Icons.AutoMirrored.Outlined.Send
    "drafts" -> Icons.Outlined.Drafts
    "archive" -> Icons.Outlined.Archive
    "junk" -> Icons.Outlined.Report
    "trash" -> Icons.Outlined.Delete
    "flagged" -> Icons.Outlined.StarOutline
    else -> Icons.Outlined.Folder
}

/** Boîte mail : dossiers, recherche, filtres, écriture, actions par glissement. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MailScreen(state: AppState, onBack: () -> Unit, onMessage: (Long, String) -> Unit, onCompose: () -> Unit, onAccounts: (String?) -> Unit) {
    val ctx = LocalContext.current
    val accounts = state.mailAccounts.data
    val account = state.currentMailAccount
    var menu by remember { mutableStateOf(false) }
    var foldersSheet by remember { mutableStateOf(false) }
    var searching by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf(state.mailSearch) }
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val trashMsg = stringResource(R.string.mail_moved_trash)
    val deletedMsg = stringResource(R.string.mail_deleted_forever)
    LaunchedEffect(Unit) { state.loadMailAccounts() }
    LaunchedEffect(account?.id) { if (account != null) { state.loadMail(); state.loadMailFolders() } }
    val folder = state.currentFolder
    val inbox = state.mailFolder.equals("INBOX", true)

    Scaffold(
        topBar = {
            if (searching) {
                val focus = remember { FocusRequester() }
                LaunchedEffect(Unit) { focus.requestFocus() }
                TopAppBar(
                    title = {
                        TextField(query, { query = it }, singleLine = true, placeholder = { Text(stringResource(R.string.search_mail)) },
                            colors = TextFieldDefaults.colors(focusedContainerColor = Color.Transparent, unfocusedContainerColor = Color.Transparent, focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent),
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search), keyboardActions = KeyboardActions(onSearch = { state.searchMail(query) }),
                            modifier = Modifier.fillMaxWidth().focusRequester(focus))
                    },
                    navigationIcon = { IconButton(onClick = { searching = false; query = ""; if (state.mailSearch.isNotEmpty()) state.searchMail("") }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                    actions = { IconButton(onClick = { state.searchMail(query) }) { Icon(Icons.Outlined.Search, stringResource(R.string.search_mail)) } },
                )
            } else TopAppBar(
                title = {
                    Row(Modifier.clip(RoundedCornerShape(8.dp)).clickable(enabled = account != null) { foldersSheet = true }, verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f, fill = false)) {
                            Text(if (account == null) stringResource(R.string.mail) else folderLabel(folder, state.mailFolder), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            account?.let { Text(it.label.ifBlank { it.email }, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                        }
                        if (account != null) Icon(Icons.Filled.ArrowDropDown, stringResource(R.string.folders))
                    }
                },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    if (account != null) {
                        IconButton(onClick = { searching = true }) { Icon(Icons.Outlined.Search, stringResource(R.string.search_mail)) }
                        IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                        DropdownMenu(menu, { menu = false }) {
                            if (inbox) DropdownMenuItem(text = { Text(stringResource(R.string.mark_all_read)) }, leadingIcon = { Icon(Icons.Outlined.DoneAll, null) }, enabled = account.unread > 0,
                                onClick = { menu = false; state.readAllMail() })
                            account.webmail_url?.let { u -> DropdownMenuItem(text = { Text(stringResource(R.string.open_webmail)) }, leadingIcon = { Icon(Icons.Outlined.OpenInBrowser, null) }, onClick = { menu = false; openUrl(ctx, u) }) }
                            DropdownMenuItem(text = { Text(stringResource(R.string.manage_mail_accounts)) }, leadingIcon = { Icon(Icons.Outlined.Settings, null) },
                                onClick = { menu = false; onAccounts(null) })
                        }
                    }
                },
            )
        },
        floatingActionButton = {
            if (account != null) ExtendedFloatingActionButton(onClick = onCompose, icon = { Icon(Icons.Outlined.Edit, null) }, text = { Text(stringResource(R.string.compose)) })
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            when {
                accounts == null && state.mailAccounts.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                accounts == null -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic))
                account == null -> Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                    EmptyState(Icons.Outlined.Mail, stringResource(R.string.no_mail_account))
                    Button(onClick = { onAccounts("") }) { Text(stringResource(R.string.set_up_mail)) }
                }
                else -> {
                    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilterChip(!state.mailUnreadOnly && !state.mailFlaggedOnly, { state.setMailFilter(false) }, label = { Text(stringResource(R.string.mail_all)) })
                        val unread = if (inbox) account.unread else folder?.unread ?: 0
                        FilterChip(state.mailUnreadOnly, { state.setMailFilter(true) }, label = { Text(if (unread > 0) stringResource(R.string.mail_unread_n, unread) else stringResource(R.string.mail_unread)) })
                        FilterChip(state.mailFlaggedOnly, { state.setMailFilter(false, flagged = true) }, label = { Text(stringResource(R.string.mail_starred)) },
                            leadingIcon = { Icon(Icons.Outlined.StarOutline, null, Modifier.size(18.dp)) })
                    }
                    if (state.mailSearch.isNotBlank()) Text(stringResource(R.string.mail_search_results, state.mailSearch), style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
                    account.last_error?.let {
                        Surface(color = MaterialTheme.colorScheme.errorContainer, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)) {
                            Text(stringResource(R.string.mail_error, if (it.startsWith("mail_")) errorText(ctx, it).trimEnd('.') else it), color = MaterialTheme.colorScheme.onErrorContainer, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(12.dp))
                        }
                    }
                    MailList(state, outgoing = folder?.role == "sent" || folder?.role == "drafts", onMessage = { onMessage(it, state.mailFolder) }, onDelete = { uid ->
                        val f = state.mailFolder
                        scope.launch {
                            val permanent = state.deleteMail(uid, f) ?: return@launch
                            snackbar.showSnackbar(if (permanent) deletedMsg else trashMsg)
                        }
                    })
                }
            }
        }
    }
    if (foldersSheet) FoldersSheet(state, onDismiss = { foldersSheet = false }, onAccounts = { foldersSheet = false; onAccounts(null) })
}

/** Dossiers du compte (et choix du compte) ; appui long : renommer, sous-dossier, vider, supprimer. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
private fun FoldersSheet(state: AppState, onDismiss: () -> Unit, onAccounts: () -> Unit) {
    val scope = rememberCoroutineScope()
    val folders = state.mailFolders
    var menuFor by remember { mutableStateOf<MailFolder?>(null) }
    var newIn by remember { mutableStateOf<String?>(null) }
    var newDialog by remember { mutableStateOf(false) }
    var rename by remember { mutableStateOf<MailFolder?>(null) }
    var delete by remember { mutableStateOf<MailFolder?>(null) }
    var empty by remember { mutableStateOf<MailFolder?>(null) }
    LaunchedEffect(Unit) { state.loadMailFolders() }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
            val accounts = state.mailAccounts.data.orEmpty()
            if (accounts.size > 1) {
                Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    accounts.forEach { a ->
                        FilterChip(a.id == state.currentMailAccount?.id, { state.selectMailAccount(a.id) }, label = { Text(a.label.ifBlank { a.email }) },
                            trailingIcon = if (a.unread > 0) { { Badge { Text(a.unread.toString()) } } } else null)
                    }
                }
            }
            Text(stringResource(R.string.folders), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            if (folders == null) Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp)) }
            val byName = folders?.folders.orEmpty().associateBy { it.name }
            fun depth(f: MailFolder): Int { var d = 0; var p = f.parent; while (p != null && d < 6) { d++; p = byName[p]?.parent }; return d }
            folders?.folders.orEmpty().forEach { f ->
                val selected = f.name == state.mailFolder
                Box {
                    ListItem(
                        colors = if (selected) ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.secondaryContainer) else sheetItemColors(),
                        headlineContent = { Text(folderLabel(f), maxLines = 1, overflow = TextOverflow.Ellipsis, color = if (f.selectable) Color.Unspecified else MaterialTheme.colorScheme.onSurfaceVariant) },
                        leadingContent = { Icon(folderIcon(f.role ?: if (f.name.equals("INBOX", true)) "inbox" else null), null) },
                        trailingContent = { if (f.unread > 0) Text("${f.unread}", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary) },
                        modifier = Modifier.padding(start = (depth(f) * 20).dp).combinedClickable(
                            enabled = true,
                            onClick = { if (f.selectable) { state.selectMailFolder(f.name); onDismiss() } },
                            onLongClick = { menuFor = f },
                        ),
                    )
                    DropdownMenu(expanded = menuFor == f, onDismissRequest = { menuFor = null }) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.new_subfolder)) }, leadingIcon = { Icon(Icons.Outlined.CreateNewFolder, null) }, onClick = { menuFor = null; newIn = f.name; newDialog = true })
                        if (!f.name.equals("INBOX", true)) DropdownMenuItem(text = { Text(stringResource(R.string.rename)) }, leadingIcon = { Icon(Icons.Outlined.Edit, null) }, onClick = { menuFor = null; rename = f })
                        if (f.role == "trash" || f.role == "junk") DropdownMenuItem(text = { Text(stringResource(R.string.empty_folder)) }, leadingIcon = { Icon(Icons.Outlined.DeleteSweep, null) }, onClick = { menuFor = null; empty = f })
                        if (!f.name.equals("INBOX", true) && f.role == null) DropdownMenuItem(text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                            leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) }, onClick = { menuFor = null; delete = f })
                    }
                }
            }
            HorizontalDivider(Modifier.padding(vertical = 8.dp))
            ListItem(colors = sheetItemColors(), headlineContent = { Text(stringResource(R.string.new_folder)) }, leadingContent = { Icon(Icons.Outlined.CreateNewFolder, null) },
                modifier = Modifier.clickable { newIn = null; newDialog = true })
            ListItem(colors = sheetItemColors(), headlineContent = { Text(stringResource(R.string.manage_mail_accounts)) }, leadingContent = { Icon(Icons.Outlined.Settings, null) },
                modifier = Modifier.clickable(onClick = onAccounts))
        }
    }
    if (newDialog) NameDialog(stringResource(if (newIn != null) R.string.new_subfolder else R.string.new_folder), stringResource(R.string.folder_name), "", onDismiss = { newDialog = false }) { label ->
        newDialog = false
        val parent = newIn
        scope.launch { state.createMailFolder(label, parent)?.let { state.selectMailFolder(it); onDismiss() } }
    }
    rename?.let { f ->
        NameDialog(stringResource(R.string.rename), stringResource(R.string.folder_name), f.label, onDismiss = { rename = null }) { label ->
            rename = null; scope.launch { state.renameMailFolder(f.name, label) }
        }
    }
    delete?.let { f ->
        AlertDialog(
            onDismissRequest = { delete = null },
            title = { Text(stringResource(R.string.delete_folder_title, f.label)) },
            text = { Text(stringResource(R.string.delete_folder_message)) },
            confirmButton = { TextButton(onClick = { delete = null; scope.launch { state.deleteMailFolder(f.name) } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { delete = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    empty?.let { f ->
        val label = folderLabel(f)
        AlertDialog(
            onDismissRequest = { empty = null },
            title = { Text(stringResource(R.string.empty_folder_title, label)) },
            text = { Text(stringResource(R.string.empty_folder_message)) },
            confirmButton = { TextButton(onClick = { empty = null; scope.launch { state.emptyMailFolder(f.name) } }) { Text(stringResource(R.string.empty_folder), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { empty = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MailList(state: AppState, outgoing: Boolean, onMessage: (Long) -> Unit, onDelete: (Long) -> Unit) {
    val list = state.mailList.data
    val listState = rememberLazyListState()
    val atEnd by remember { derivedStateOf { listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index?.let { it >= listState.layoutInfo.totalItemsCount - 3 } ?: false } }
    LaunchedEffect(atEnd, state.mailHasMore) { if (atEnd && state.mailHasMore) state.loadMoreMail() }
    PullToRefreshBox(isRefreshing = state.mailList.loading && list != null, onRefresh = { state.refreshMail() }, modifier = Modifier.fillMaxSize()) {
        when {
            list == null && state.mailList.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            list == null -> EmptyState(Icons.Outlined.CloudOff, errorText(LocalContext.current, state.mailList.error))
            list.isEmpty() -> LazyColumn(Modifier.fillMaxSize()) { item { EmptyState(Icons.Outlined.Inbox, stringResource(if (state.mailUnreadOnly) R.string.no_unread_mail else R.string.no_mail)) } }
            else -> LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = PaddingValues(bottom = 96.dp)) {
                items(list, key = { it.uid }) { m ->
                    MailRow(m, outgoing, onClick = { onMessage(m.uid) }, onToggle = { state.setSeen(m.uid, !m.seen) }, onDelete = { onDelete(m.uid) }, onStar = { state.setFlagged(m.uid, !m.flagged) })
                }
                if (state.mailLoadingMore) item { Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp)) } }
            }
        }
    }
}

/** Glisser vers la droite : lu / non lu ; vers la gauche : corbeille. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MailRow(m: MailSummary, outgoing: Boolean, onClick: () -> Unit, onToggle: () -> Unit, onDelete: () -> Unit, onStar: () -> Unit) {
    // Refuser l'état « balayé » : la ligne revient en place après l'action.
    val toggle by rememberUpdatedState(onToggle)
    val delete by rememberUpdatedState(onDelete)
    @Suppress("DEPRECATION")
    val swipe = rememberSwipeToDismissBoxState(confirmValueChange = {
        when (it) { SwipeToDismissBoxValue.StartToEnd -> toggle(); SwipeToDismissBoxValue.EndToStart -> delete(); else -> {} }
        false
    })
    SwipeToDismissBox(
        swipe,
        backgroundContent = {
            val toTrash = swipe.dismissDirection == SwipeToDismissBoxValue.EndToStart
            Row(Modifier.fillMaxSize().background(if (toTrash) MaterialTheme.colorScheme.errorContainer else MaterialTheme.colorScheme.secondaryContainer).padding(horizontal = 24.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = if (toTrash) Arrangement.End else Arrangement.Start) {
                if (toTrash) {
                    Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.onErrorContainer)
                    Spacer(Modifier.width(12.dp))
                    Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.onErrorContainer)
                } else {
                    Icon(if (m.seen) Icons.Outlined.MarkEmailUnread else Icons.Outlined.MarkEmailRead, null, tint = MaterialTheme.colorScheme.onSecondaryContainer)
                    Spacer(Modifier.width(12.dp))
                    Text(stringResource(if (m.seen) R.string.mark_unread else R.string.mark_read), color = MaterialTheme.colorScheme.onSecondaryContainer)
                }
            }
        },
    ) {
        val weight = if (m.seen) FontWeight.Normal else FontWeight.Bold
        val who = if (outgoing && m.to.isNotEmpty()) m.to.joinToString { it.substringBefore('<').trim().ifBlank { it } } else m.from_name.ifBlank { m.from_addr }
        ListItem(
            headlineContent = {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(who, Modifier.weight(1f), fontWeight = weight, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (m.has_attachments) Icon(Icons.Outlined.AttachFile, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    if (m.answered) Icon(Icons.AutoMirrored.Outlined.Reply, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(mailTime(m.sent_at), style = MaterialTheme.typography.labelMedium, fontWeight = weight,
                        color = if (m.seen) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 8.dp))
                }
            },
            supportingContent = {
                Row(verticalAlignment = Alignment.Top) {
                    Column(Modifier.weight(1f)) {
                        Text(m.subject.ifBlank { stringResource(R.string.no_subject) }, fontWeight = weight, color = MaterialTheme.colorScheme.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (m.snippet.isNotBlank()) Text(m.snippet, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    }
                    IconButton(onClick = onStar, modifier = Modifier.size(32.dp)) {
                        Icon(if (m.flagged) Icons.Filled.Star else Icons.Outlined.StarOutline, stringResource(if (m.flagged) R.string.unstar else R.string.star),
                            Modifier.size(20.dp), tint = if (m.flagged) Color(0xFFEDA100) else MaterialTheme.colorScheme.outline)
                    }
                }
            },
            leadingContent = { Avatar(who, 40.dp) },
            modifier = Modifier.clickable(onClick = onClick),
        )
    }
}

private fun MailAddress.label() = name?.takeIf { it.isNotBlank() } ?: address.orEmpty()

/** Lecture d'un message : HTML nettoyé par le serveur, sans JavaScript, images distantes bloquées. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MessageScreen(state: AppState, uid: Long, folder: String, onBack: () -> Unit, onCompose: (String) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var msg by remember { mutableStateOf<MailMessage?>(null) }
    var failed by remember { mutableStateOf(false) }
    var images by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var moveSheet by remember { mutableStateOf(false) }
    var opening by remember { mutableStateOf<Int?>(null) }
    val summary = state.mailList.data?.firstOrNull { it.uid == uid }
    LaunchedEffect(uid, folder) { msg = state.readMail(uid, folder); failed = msg == null }
    // Suivi modifié depuis cet écran : la copie partagée fait foi.
    val m = msg?.let { cur -> state.openMail?.takeIf { it.uid == cur.uid && it.folder == cur.folder } ?: cur }
    val canSend = state.currentMailAccount?.can_send == true || state.mailAccounts.data.orEmpty().any { it.can_send }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    if (m != null) {
                        IconButton(onClick = { state.setFlagged(uid, !m.flagged, folder) }) {
                            Icon(if (m.flagged) Icons.Filled.Star else Icons.Outlined.StarOutline, stringResource(if (m.flagged) R.string.unstar else R.string.star), tint = if (m.flagged) Color(0xFFEDA100) else LocalContentColor.current)
                        }
                        IconButton(onClick = { scope.launch { state.deleteMail(uid, folder); onBack() } }) { Icon(Icons.Outlined.Delete, stringResource(R.string.delete)) }
                    }
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                    DropdownMenu(menu, { menu = false }) {
                        if (m != null && canSend) DropdownMenuItem(text = { Text(stringResource(R.string.reply_all)) }, leadingIcon = { Icon(Icons.AutoMirrored.Outlined.ReplyAll, null) }, onClick = { menu = false; onCompose("replyAll") })
                        if (m != null) DropdownMenuItem(text = { Text(stringResource(R.string.move_to_folder)) }, leadingIcon = { Icon(Icons.Outlined.DriveFileMove, null) }, onClick = { menu = false; moveSheet = true })
                        DropdownMenuItem(text = { Text(stringResource(R.string.mark_unread)) }, leadingIcon = { Icon(Icons.Outlined.MarkEmailUnread, null) }, onClick = { menu = false; state.setSeen(uid, false, folder); onBack() })
                        state.currentMailAccount?.webmail_url?.let { u -> DropdownMenuItem(text = { Text(stringResource(R.string.open_webmail)) }, leadingIcon = { Icon(Icons.Outlined.OpenInBrowser, null) }, onClick = { menu = false; openUrl(ctx, u) }) }
                    }
                },
            )
        },
        bottomBar = {
            if (m != null && canSend) Surface(tonalElevation = 2.dp) {
                Row(Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 12.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(onClick = { onCompose("reply") }, modifier = Modifier.weight(1f)) { Icon(Icons.AutoMirrored.Outlined.Reply, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.reply)) }
                    OutlinedButton(onClick = { onCompose("forward") }, modifier = Modifier.weight(1f)) { Icon(Icons.Outlined.Forward, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.forward)) }
                }
            }
        },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            Text((m?.subject ?: summary?.subject)?.ifBlank { null } ?: stringResource(R.string.no_subject), style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
            val from = m?.from?.firstOrNull()
            val name = from?.label() ?: summary?.from_name?.ifBlank { summary.from_addr }
            ListItem(
                headlineContent = { Text(name.orEmpty(), fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                supportingContent = {
                    Column {
                        from?.address?.takeIf { it != name }?.let { Text(it, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                        if (m != null && m.to.isNotEmpty()) Text(stringResource(R.string.mail_to, m.to.joinToString { it.label() }), maxLines = 2, overflow = TextOverflow.Ellipsis)
                        if (m != null && m.cc.isNotEmpty()) Text(stringResource(R.string.mail_cc, m.cc.joinToString { it.label() }), maxLines = 2, overflow = TextOverflow.Ellipsis)
                    }
                },
                leadingContent = { Avatar(name, 40.dp) },
                trailingContent = { Text(mailTime(m?.date ?: summary?.sent_at), style = MaterialTheme.typography.labelMedium) },
            )
            if (m != null && m.attachments.isNotEmpty()) {
                Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    m.attachments.forEach { a ->
                        AssistChip(
                            enabled = opening == null && a.index >= 0,
                            onClick = {
                                opening = a.index
                                scope.launch { state.mailAttachmentFile(m, a)?.let { openFile(ctx, it, a.type) }; opening = null }
                            },
                            label = { Text("${a.name} · ${android.text.format.Formatter.formatShortFileSize(ctx, a.size)}", maxLines = 1) },
                            leadingIcon = { if (opening == a.index) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Icon(Icons.Outlined.AttachFile, null, Modifier.size(18.dp)) },
                        )
                    }
                }
            }
            val html = m?.html
            if (html != null && !images && Regex("<img[^>]+src=[\"']?https?:", RegexOption.IGNORE_CASE).containsMatchIn(html)) {
                Surface(color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)) {
                    Row(Modifier.padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.HideImage, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.width(8.dp))
                        Text(stringResource(R.string.images_blocked), Modifier.weight(1f), style = MaterialTheme.typography.bodySmall)
                        TextButton(onClick = { images = true }) { Text(stringResource(R.string.show_images)) }
                    }
                }
            }
            HorizontalDivider(Modifier.padding(top = 8.dp))
            when {
                m == null && !failed -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                m == null -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic))
                html != null -> MailHtml(html, images, Modifier.fillMaxSize())
                else -> SelectionContainer {
                    Text(linkify(m.text.orEmpty(), MaterialTheme.colorScheme.primary), Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
    if (moveSheet) MoveSheet(state, folder, onDismiss = { moveSheet = false }) { to -> moveSheet = false; state.moveMail(uid, folder, to); onBack() }
}

/** Choix du dossier où déplacer un message. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MoveSheet(state: AppState, current: String, onDismiss: () -> Unit, onPick: (String) -> Unit) {
    LaunchedEffect(Unit) { if (state.mailFolders == null) state.loadMailFolders() }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
            Text(stringResource(R.string.move_to_folder), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            state.mailFolders?.folders.orEmpty().filter { it.selectable && it.name != current }.forEach { f ->
                ListItem(colors = sheetItemColors(), headlineContent = { Text(folderLabel(f)) }, leadingContent = { Icon(folderIcon(f.role ?: if (f.name.equals("INBOX", true)) "inbox" else null), null) },
                    modifier = Modifier.clickable { onPick(f.name) })
            }
        }
    }
}

@SuppressLint("SetJavaScriptEnabled")
@Composable
private fun MailHtml(html: String, images: Boolean, modifier: Modifier) {
    // Fond blanc comme un client de messagerie : les messages HTML supposent souvent un fond clair.
    val doc = """<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer">
        <style>body{margin:12px;font-family:sans-serif;font-size:15px;line-height:1.45;color:#1a1a1a;background:#fff;word-wrap:break-word}img{max-width:100%;height:auto}table{max-width:100%!important}pre{white-space:pre-wrap}</style>
        </head><body>$html</body></html>"""
    AndroidView(
        factory = { c ->
            WebView(c).apply {
                settings.javaScriptEnabled = false
                settings.allowFileAccess = false
                settings.allowContentAccess = false
                settings.domStorageEnabled = false
                settings.loadWithOverviewMode = true
                settings.useWideViewPort = true
                settings.builtInZoomControls = true
                settings.displayZoomControls = false
                setBackgroundColor(android.graphics.Color.WHITE)
                webViewClient = object : WebViewClient() {
                    // Message trop lourd : la vue tombe, pas l'application.
                    override fun onRenderProcessGone(view: WebView, detail: android.webkit.RenderProcessGoneDetail): Boolean = true
                    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                        openUrl(view.context, request.url.toString())
                        return true
                    }
                }
            }
        },
        update = { w ->
            w.settings.blockNetworkImage = !images
            w.settings.blockNetworkLoads = !images
            val key = "$images$doc"
            if (w.tag != key) { w.tag = key; w.loadDataWithBaseURL(null, doc, "text/html", "utf-8", null) }
        },
        modifier = modifier,
    )
}

private val URL_RE = Regex("""https?://[^\s<>"')\]]+""")

/** Texte brut : adresses web cliquables (ouvertes dans le navigateur). */
private fun linkify(text: String, color: androidx.compose.ui.graphics.Color): androidx.compose.ui.text.AnnotatedString =
    androidx.compose.ui.text.buildAnnotatedString {
        var last = 0
        val style = androidx.compose.ui.text.TextLinkStyles(androidx.compose.ui.text.SpanStyle(color = color, textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline))
        URL_RE.findAll(text).forEach { m ->
            append(text.substring(last, m.range.first))
            withLink(androidx.compose.ui.text.LinkAnnotation.Url(m.value.trimEnd('.', ','), style)) { append(m.value) }
            last = m.range.last + 1
        }
        append(text.substring(last))
    }
