package io.github.untypequicode.looty.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.text.format.Formatter
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.DriveItem
import io.github.untypequicode.looty.api.DriveList
import kotlinx.coroutines.launch
import java.time.Duration
import java.time.Instant

private fun join(folder: String, name: String) = if (folder.isEmpty()) name else "$folder/$name"

/** Jours restants avant l'effacement automatique. */
private fun daysLeft(iso: String?): Long? = iso?.let { runCatching { Duration.between(Instant.now(), Instant.parse(it)).toHours() }.getOrNull() }?.let { (it + 23) / 24 }

fun mimeIcon(mime: String?, name: String = "") = when {
    mime?.startsWith("image/") == true -> Icons.Outlined.Image
    mime == "application/pdf" || name.endsWith(".pdf", true) -> Icons.Outlined.PictureAsPdf
    mime?.startsWith("video/") == true -> Icons.Outlined.Movie
    mime?.startsWith("audio/") == true -> Icons.Outlined.AudioFile
    mime?.contains("zip") == true || name.endsWith(".zip", true) -> Icons.Outlined.FolderZip
    mime?.startsWith("text/") == true -> Icons.Outlined.Description
    else -> Icons.Outlined.InsertDriveFile
}

/**
 * Drive de l'espace : fichiers temporaires (effacés automatiquement), dossiers, envoi depuis
 * le téléphone, ouverture, lien de téléchargement, renommer, déplacer, prolonger, supprimer.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DriveScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var folder by rememberSaveableString("")
    var query by remember { mutableStateOf<String?>(null) }
    var data by remember { mutableStateOf<DriveList?>(null) }
    var loading by remember { mutableStateOf(false) }
    var actionsFor by remember { mutableStateOf<DriveItem?>(null) }
    var newFolder by remember { mutableStateOf(false) }
    var pendingUris by remember { mutableStateOf<List<android.net.Uri>>(emptyList()) }
    var uploading by remember { mutableIntStateOf(0) }
    var opening by remember { mutableStateOf<String?>(null) }
    val snackbar = remember { SnackbarHostState() }
    val res = ctx.resources
    fun reload() { scope.launch { loading = true; data = state.driveList(folder, query.orEmpty()) ?: data; loading = false } }
    LaunchedEffect(folder, query) { reload() }
    BackHandler(enabled = folder.isNotEmpty() && query == null) { folder = folder.substringBeforeLast('/', "") }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { if (it.isNotEmpty()) pendingUris = it }

    Scaffold(
        topBar = {
            if (query != null) {
                val focus = remember { FocusRequester() }
                LaunchedEffect(Unit) { focus.requestFocus() }
                TopAppBar(
                    title = {
                        TextField(query!!, { query = it }, singleLine = true, placeholder = { Text(stringResource(R.string.search_files)) },
                            colors = TextFieldDefaults.colors(focusedContainerColor = Color.Transparent, unfocusedContainerColor = Color.Transparent, focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent),
                            keyboardOptions = androidx.compose.foundation.text.KeyboardOptions(imeAction = ImeAction.Search), modifier = Modifier.fillMaxWidth().focusRequester(focus))
                    },
                    navigationIcon = { IconButton(onClick = { query = null }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                )
            } else TopAppBar(
                title = { Text(stringResource(R.string.drive)) },
                navigationIcon = { IconButton(onClick = { if (folder.isNotEmpty()) folder = folder.substringBeforeLast('/', "") else onBack() }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    IconButton(onClick = { query = "" }) { Icon(Icons.Outlined.Search, stringResource(R.string.search_files)) }
                    if (data?.can_upload == true) IconButton(onClick = { newFolder = true }) { Icon(Icons.Outlined.CreateNewFolder, stringResource(R.string.new_folder)) }
                },
            )
        },
        floatingActionButton = {
            if (data?.can_upload == true && query == null) ExtendedFloatingActionButton(onClick = { pick.launch(arrayOf("*/*")) }, icon = { Icon(Icons.Outlined.Upload, null) }, text = { Text(stringResource(R.string.upload)) })
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            // Chemin du dossier
            if (query == null) Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                val parts = if (folder.isEmpty()) emptyList() else folder.split('/')
                TextButton(onClick = { folder = "" }) { Icon(Icons.Outlined.Home, null, Modifier.size(18.dp)); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.drive)) }
                parts.forEachIndexed { i, p ->
                    Icon(Icons.Outlined.ChevronRight, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    TextButton(onClick = { folder = parts.take(i + 1).joinToString("/") }) { Text(p, maxLines = 1) }
                }
            }
            data?.let { d ->
                if (d.quota_bytes > 0) Column(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
                    LinearProgressIndicator({ (d.used_bytes.toFloat() / d.quota_bytes).coerceIn(0f, 1f) }, Modifier.fillMaxWidth())
                    Text(stringResource(R.string.drive_usage, Formatter.formatShortFileSize(ctx, d.used_bytes), Formatter.formatShortFileSize(ctx, d.quota_bytes), d.max_days),
                        style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 4.dp))
                }
            }
            if (uploading > 0) LinearProgressIndicator(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp))
            PullToRefreshBox(isRefreshing = loading && data != null, onRefresh = { reload() }, modifier = Modifier.fillMaxSize()) {
                val d = data
                when {
                    d == null && loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                    d == null -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic))
                    else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
                        val items = d.items.sortedWith(compareBy({ !it.is_dir }, { it.name.lowercase() }))
                        if (items.isEmpty()) item { EmptyState(Icons.Outlined.FolderOpen, stringResource(if (query.isNullOrBlank()) R.string.drive_empty else R.string.no_results)) }
                        items(items, key = { it.id }) { f ->
                            val left = daysLeft(f.expires_at)
                            ListItem(
                                headlineContent = { Text(f.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                supportingContent = {
                                    Text(if (f.is_dir) pluralStringResource(R.plurals.n_items, f.children.toInt(), f.children.toInt())
                                    else listOfNotNull(
                                        Formatter.formatShortFileSize(ctx, f.size),
                                        left?.let { pluralStringResource(R.plurals.expires_in_days, it.toInt().coerceAtLeast(0), it.toInt().coerceAtLeast(0)) },
                                        f.folder.takeIf { !query.isNullOrBlank() && it.isNotEmpty() },
                                        f.uploaded_by_name,
                                    ).joinToString(" · "), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                },
                                leadingContent = {
                                    if (opening == f.id) CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                                    else Icon(if (f.is_dir) Icons.Outlined.Folder else mimeIcon(f.mime, f.name), null, tint = if (f.is_dir) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
                                },
                                trailingContent = {
                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        if (f.shared) Icon(Icons.Outlined.Link, stringResource(R.string.public_link), Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
                                        IconButton(onClick = { actionsFor = f }) { Icon(Icons.Outlined.MoreVert, null) }
                                    }
                                },
                                modifier = Modifier.clickable(enabled = opening == null) {
                                    if (f.is_dir) { query = null; folder = join(f.folder, f.name) }
                                    else { opening = f.id; scope.launch { state.driveFile(f)?.let { openFile(ctx, it, f.mime) }; opening = null } }
                                },
                            )
                        }
                    }
                }
            }
        }
    }

    if (pendingUris.isNotEmpty()) data?.let { d ->
        DaysDialog(stringResource(R.string.keep_files_for), d.day_choices.filter { it <= d.max_days }.ifEmpty { listOf(d.max_days) }, d.max_days, onDismiss = { pendingUris = emptyList() }) { days ->
            val uris = pendingUris; pendingUris = emptyList(); uploading++
            val target = folder
            state.launch {
                val failed = state.driveUpload(target, days, uris)
                uploading--
                if (failed > 0) snackbar.showSnackbar(res.getQuantityString(R.plurals.upload_failed, failed, failed))
                if (folder == target) reload()
            }
        }
    }
    if (newFolder) NameDialog(stringResource(R.string.new_folder), stringResource(R.string.folder_name), "", onDismiss = { newFolder = false }) { name ->
        newFolder = false; scope.launch { if (state.driveCreateFolder(folder, name)) reload() }
    }
    actionsFor?.let { f -> DriveActions(state, f, data, onDismiss = { actionsFor = null }, onChanged = { reload() }) }
}

@Composable
private fun rememberSaveableString(initial: String) = androidx.compose.runtime.saveable.rememberSaveable { mutableStateOf(initial) }

/** Choix d'une durée de conservation (jours). */
@Composable
private fun DaysDialog(title: String, choices: List<Int>, initial: Int, onDismiss: () -> Unit, onPick: (Int) -> Unit) {
    var days by remember { mutableIntStateOf(if (initial in choices) initial else choices.last()) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                choices.forEach { d ->
                    Row(Modifier.fillMaxWidth().clickable { days = d }.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(days == d, { days = d })
                        Text(pluralStringResource(R.plurals.n_days, d, d))
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = { onPick(days) }) { Text(stringResource(R.string.ok)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Actions d'un fichier ou d'un dossier du drive. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DriveActions(state: AppState, f: DriveItem, data: DriveList?, onDismiss: () -> Unit, onChanged: () -> Unit) {
    val ctx = LocalContext.current
    var dialog by remember { mutableStateOf<String?>(null) }
    var link by remember { mutableStateOf<String?>(null) }
    val mine = f.uploaded_by == data?.user_id
    val canChange = data?.can_upload == true && (mine || data.can_manage)
    val copied = stringResource(R.string.link_copied)
    if (dialog == null) ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
            Row(Modifier.padding(horizontal = 24.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(if (f.is_dir) Icons.Outlined.Folder else mimeIcon(f.mime, f.name), null); Spacer(Modifier.width(12.dp))
                Text(f.name, style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            @Composable
            fun item(icon: androidx.compose.ui.graphics.vector.ImageVector, label: Int, danger: Boolean = false, onClick: () -> Unit) = ListItem(
                colors = sheetItemColors(),
                headlineContent = { Text(stringResource(label), color = if (danger) MaterialTheme.colorScheme.error else Color.Unspecified) },
                leadingContent = { Icon(icon, null, tint = if (danger) MaterialTheme.colorScheme.error else LocalContentColor.current) },
                modifier = Modifier.clickable(onClick = onClick),
            )
            if (!f.is_dir) {
                item(Icons.Outlined.Link, if (f.shared) R.string.copy_link else R.string.create_link) {
                    state.launch {
                        state.driveShare(f.id, true)?.let { l ->
                            ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(f.name, l.url))
                            Toast.makeText(ctx, copied, Toast.LENGTH_SHORT).show()
                            link = l.url; dialog = "link"; onChanged()
                        }
                    }
                }
                if (f.shared && canChange) item(Icons.Outlined.LinkOff, R.string.remove_link) { onDismiss(); state.launch { state.driveShare(f.id, false); onChanged() } }
            }
            if (canChange) {
                item(Icons.Outlined.DriveFileRenameOutline, R.string.rename) { dialog = "rename" }
                item(Icons.Outlined.DriveFileMove, R.string.move_to_folder) { dialog = "move" }
                if (!f.is_dir) item(Icons.Outlined.Schedule, R.string.keep_longer) { dialog = "days" }
                item(Icons.Outlined.Delete, R.string.delete, danger = true) { dialog = "delete" }
            }
        }
    }
    when (dialog) {
        "link" -> AlertDialog(
            onDismissRequest = onDismiss,
            title = { Text(stringResource(R.string.public_link)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.drive_link_hint), style = MaterialTheme.typography.bodySmall)
                    Text(link.orEmpty(), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.primary)
                }
            },
            confirmButton = { TextButton(onClick = { onDismiss(); ctx.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link), null)) }) { Text(stringResource(R.string.send_link)) } },
            dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.ok)) } },
        )
        "rename" -> NameDialog(stringResource(R.string.rename), stringResource(R.string.label_name), f.name, onDismiss = onDismiss) { n -> onDismiss(); state.launch { if (state.driveUpdate(f.id, name = n)) onChanged() } }
        "move" -> DriveFolderPicker(state, exclude = if (f.is_dir) join(f.folder, f.name) else null, current = f.folder, onDismiss = onDismiss) { target -> onDismiss(); state.launch { if (state.driveUpdate(f.id, folder = target)) onChanged() } }
        "days" -> data?.let { d ->
            DaysDialog(stringResource(R.string.keep_longer), d.day_choices.filter { it <= d.max_days }.ifEmpty { listOf(d.max_days) }, d.max_days, onDismiss = onDismiss) { days -> onDismiss(); state.launch { if (state.driveUpdate(f.id, days = days)) onChanged() } }
        }
        "delete" -> AlertDialog(
            onDismissRequest = onDismiss,
            title = { Text(stringResource(R.string.delete_attachment_title, f.name)) },
            text = { if (f.is_dir) Text(stringResource(R.string.delete_drive_folder_hint)) },
            confirmButton = { TextButton(onClick = { onDismiss(); state.launch { if (state.driveDelete(f.id)) onChanged() } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** Choix d'un dossier du drive (navigation dans l'arborescence). */
@Composable
private fun DriveFolderPicker(state: AppState, exclude: String?, current: String, onDismiss: () -> Unit, onPick: (String) -> Unit) {
    var folder by remember { mutableStateOf("") }
    var subs by remember { mutableStateOf<List<String>?>(null) }
    LaunchedEffect(folder) { subs = null; subs = state.driveList(folder)?.items?.filter { it.is_dir }?.map { join(it.folder, it.name) }?.filter { it != exclude }.orEmpty() }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.move_to_folder)) },
        text = {
            Column(Modifier.heightIn(max = 400.dp).verticalScroll(rememberScrollState())) {
                Text("/" + folder, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(bottom = 8.dp))
                if (folder.isNotEmpty()) ListItem(headlineContent = { Text("..") }, leadingContent = { Icon(Icons.Outlined.ArrowUpward, null) },
                    modifier = Modifier.clickable { folder = folder.substringBeforeLast('/', "") }, colors = sheetItemColors())
                val s = subs
                if (s == null) CircularProgressIndicator(Modifier.size(24.dp))
                else s.forEach { p ->
                    ListItem(headlineContent = { Text(p.substringAfterLast('/')) }, leadingContent = { Icon(Icons.Outlined.Folder, null) },
                        modifier = Modifier.clickable { folder = p }, colors = sheetItemColors())
                }
            }
        },
        confirmButton = { TextButton(enabled = folder != current, onClick = { onPick(folder) }) { Text(stringResource(R.string.move_here)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
