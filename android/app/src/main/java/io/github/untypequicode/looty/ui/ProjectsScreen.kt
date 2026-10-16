package io.github.untypequicode.looty.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.net.Uri
import android.text.format.Formatter
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.CompileResult
import io.github.untypequicode.looty.api.ProjectFile
import io.github.untypequicode.looty.api.ProjectFull
import io.github.untypequicode.looty.api.ProjectSummary
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

private fun kindIcon(kind: String) = when (kind) {
    "latex" -> Icons.Outlined.Functions
    "notebook" -> Icons.Outlined.Terminal
    else -> Icons.Outlined.Language
}

private fun kindLabel(kind: String) = when (kind) { "latex" -> R.string.kind_latex; "notebook" -> R.string.kind_notebook; else -> R.string.kind_site }

private val TEMPLATES = mapOf(
    "site" to listOf("starter" to R.string.tpl_starter, "blank" to R.string.tpl_blank),
    "latex" to listOf("article" to R.string.tpl_article, "report" to R.string.tpl_report, "beamer" to R.string.tpl_beamer, "letter" to R.string.tpl_letter, "blank" to R.string.tpl_blank),
    "notebook" to listOf("starter" to R.string.tpl_starter, "data" to R.string.tpl_data, "blank" to R.string.tpl_blank),
)

/** Projets de l'espace : mini-sites, LaTeX, notebooks. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProjectsScreen(state: AppState, onBack: () -> Unit, onOpen: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var list by remember { mutableStateOf<List<ProjectSummary>?>(null) }
    var loading by remember { mutableStateOf(false) }
    var creating by remember { mutableStateOf(false) }
    fun reload() { scope.launch { loading = true; list = state.projects() ?: list; loading = false } }
    LaunchedEffect(Unit) { reload() }
    Scaffold(
        topBar = {
            TopAppBar(title = { Text(stringResource(R.string.projects)) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } })
        },
        floatingActionButton = { if (state.canEditDocs) ExtendedFloatingActionButton(onClick = { creating = true }, icon = { Icon(Icons.Filled.Add, null) }, text = { Text(stringResource(R.string.new_project)) }) },
    ) { padding ->
        PullToRefreshBox(isRefreshing = loading && list != null, onRefresh = { reload() }, modifier = Modifier.padding(padding).fillMaxSize()) {
            val l = list
            when {
                l == null && loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                l == null -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic))
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
                    if (l.isEmpty()) item { EmptyState(Icons.Outlined.Code, stringResource(R.string.no_projects)) }
                    items(l, key = { it.id }) { p ->
                        ListItem(
                            headlineContent = { Text(p.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                            supportingContent = {
                                Text(listOfNotNull(
                                    stringResource(kindLabel(p.kind)),
                                    pluralStringResource(R.plurals.n_files, p.files.toInt(), p.files.toInt()),
                                    if (p.kind == "site" && p.published) stringResource(R.string.published) else null,
                                    when (p.last_build_ok) { true -> stringResource(R.string.build_ok); false -> stringResource(R.string.build_failed); null -> null }.takeIf { p.kind == "latex" },
                                ).joinToString(" · "), maxLines = 1)
                            },
                            leadingContent = { Icon(kindIcon(p.kind), null, tint = MaterialTheme.colorScheme.primary) },
                            trailingContent = { Icon(Icons.Outlined.ChevronRight, null) },
                            modifier = Modifier.clickable { onOpen(p.id) },
                        )
                    }
                }
            }
        }
    }
    if (creating) NewProjectDialog(onDismiss = { creating = false }) { kind, tpl, name ->
        creating = false
        scope.launch { state.createProject(kind, name, tpl)?.let { onOpen(it) } }
    }
}

@Composable
private fun NewProjectDialog(onDismiss: () -> Unit, onCreate: (String, String, String) -> Unit) {
    var kind by remember { mutableStateOf("site") }
    var tpl by remember { mutableStateOf("starter") }
    var name by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.new_project)) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedTextField(name, { name = it }, singleLine = true, label = { Text(stringResource(R.string.label_name)) }, modifier = Modifier.fillMaxWidth())
                Row(Modifier.horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf("site", "latex", "notebook").forEach { k ->
                        FilterChip(kind == k, { kind = k; tpl = TEMPLATES[k]!!.first().first }, label = { Text(stringResource(kindLabel(k))) }, leadingIcon = { Icon(kindIcon(k), null, Modifier.size(18.dp)) })
                    }
                }
                Text(stringResource(R.string.template), style = MaterialTheme.typography.labelLarge)
                TEMPLATES[kind]!!.forEach { (t, label) ->
                    Row(Modifier.fillMaxWidth().clickable { tpl = t }, verticalAlignment = Alignment.CenterVertically) { RadioButton(tpl == t, { tpl = t }); Text(stringResource(label)) }
                }
            }
        },
        confirmButton = { TextButton(enabled = name.isNotBlank(), onClick = { onCreate(kind, tpl, name) }) { Text(stringResource(R.string.create)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

/**
 * Un projet : fichiers (voir, ajouter, envoyer, renommer, supprimer), compilation LaTeX et PDF,
 * aperçu et publication d'un site, archive. Le texte se modifie dans l'éditeur en direct du site.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProjectScreen(state: AppState, id: String, onBack: () -> Unit, onFile: (String) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var data by remember { mutableStateOf<ProjectFull?>(null) }
    var failed by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    var dialog by remember { mutableStateOf<String?>(null) }
    var fileFor by remember { mutableStateOf<ProjectFile?>(null) }
    var busy by remember { mutableStateOf<String?>(null) }
    var build by remember { mutableStateOf<CompileResult?>(null) }
    var showLog by remember { mutableStateOf(false) }
    val snackbar = remember { SnackbarHostState() }
    val res = ctx.resources
    val copied = stringResource(R.string.link_copied)
    fun reload() { scope.launch { val r = state.project(id); if (r != null) data = r else if (data == null) failed = true } }
    LaunchedEffect(id) { reload() }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isEmpty()) return@rememberLauncherForActivityResult
        busy = "upload"
        state.launch {
            val failedN = state.projectUpload(id, "", uris)
            busy = null
            if (failedN > 0) snackbar.showSnackbar(res.getQuantityString(R.plurals.upload_failed, failedN, failedN))
            reload()
        }
    }
    val d = data
    val p = d?.project
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(p?.name.orEmpty(), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    if (d != null) {
                        IconButton(onClick = { state.ws?.let { openWeb(ctx, "/w/$it/projects/$id") } }) { Icon(Icons.Outlined.Edit, stringResource(R.string.edit_in_editor)) }
                        IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                        DropdownMenu(menu, { menu = false }) {
                            if (d.can_write) {
                                DropdownMenuItem(text = { Text(stringResource(R.string.new_file)) }, leadingIcon = { Icon(Icons.Outlined.NoteAdd, null) }, onClick = { menu = false; dialog = "file" })
                                DropdownMenuItem(text = { Text(stringResource(R.string.new_folder)) }, leadingIcon = { Icon(Icons.Outlined.CreateNewFolder, null) }, onClick = { menu = false; dialog = "folder" })
                                DropdownMenuItem(text = { Text(stringResource(R.string.upload)) }, leadingIcon = { Icon(Icons.Outlined.Upload, null) }, onClick = { menu = false; pick.launch(arrayOf("*/*")) })
                                DropdownMenuItem(text = { Text(stringResource(R.string.rename)) }, leadingIcon = { Icon(Icons.Outlined.DriveFileRenameOutline, null) }, onClick = { menu = false; dialog = "rename" })
                            }
                            DropdownMenuItem(text = { Text(stringResource(R.string.download_zip)) }, leadingIcon = { Icon(Icons.Outlined.FolderZip, null) }, onClick = {
                                menu = false; busy = "zip"
                                state.launch { state.projectZip(id, p?.name.orEmpty())?.let { shareFile(ctx, it, "application/zip") }; busy = null }
                            })
                            if (d.can_manage) DropdownMenuItem(text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) },
                                leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) }, onClick = { menu = false; dialog = "delete" })
                        }
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        if (d == null || p == null) {
            Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { if (failed) Text(stringResource(R.string.error_generic)) else LoadingIndicatorCompat() }
            return@Scaffold
        }
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 48.dp)) {
            if (busy != null) item { LinearProgressIndicator(Modifier.fillMaxWidth()) }
            item {
                Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Icon(kindIcon(p.kind), null, tint = MaterialTheme.colorScheme.primary); Spacer(Modifier.width(8.dp))
                    Text(stringResource(kindLabel(p.kind)), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            // LaTeX : compilation et PDF
            if (p.kind == "latex") item {
                OutlinedCard(Modifier.fillMaxWidth().padding(16.dp)) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(enabled = busy == null && d.can_write, onClick = { busy = "compile"; state.launch { build = state.compileProject(id); busy = null } }) {
                                Icon(Icons.Outlined.PlayArrow, null); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.compile))
                            }
                            OutlinedButton(enabled = busy == null, onClick = { busy = "pdf"; state.launch { state.projectPdf(id, p.name)?.let { openFile(ctx, it, "application/pdf") }; busy = null } }) {
                                Icon(Icons.Outlined.PictureAsPdf, null); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.open_pdf))
                            }
                        }
                        build?.let { b ->
                            Text(if (b.ok) stringResource(R.string.compile_ok, b.duration_ms / 1000.0) else stringResource(R.string.compile_failed),
                                color = if (b.ok) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error, style = MaterialTheme.typography.titleSmall)
                            b.issues.take(20).forEach { i ->
                                Text(listOfNotNull(i.file?.let { f -> i.line?.let { "$f:$it" } ?: f }, i.message).joinToString(" — "), style = MaterialTheme.typography.bodySmall,
                                    color = if (i.level == "error") MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            TextButton(onClick = { showLog = !showLog }, contentPadding = PaddingValues(0.dp)) { Text(stringResource(R.string.show_log)) }
                            if (showLog) Text(b.log.takeLast(6000), fontFamily = FontFamily.Monospace, fontSize = 11.sp, modifier = Modifier.horizontalScroll(rememberScrollState()))
                        }
                    }
                }
            }
            // Site : aperçu et publication
            if (p.kind == "site") item {
                OutlinedCard(Modifier.fillMaxWidth().padding(16.dp)) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedButton(enabled = busy == null, onClick = { busy = "preview"; state.launch { state.projectPreview(id)?.let { openWeb(ctx, it) }; busy = null } }) {
                            Icon(Icons.Outlined.Visibility, null); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.preview))
                        }
                        if (d.can_publish) {
                            var slug by remember(p.slug) { mutableStateOf(p.slug.orEmpty()) }
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(stringResource(R.string.publish_site), Modifier.weight(1f), style = MaterialTheme.typography.titleSmall)
                                Switch(p.published, onCheckedChange = { on ->
                                    state.launch { if (state.updateProject(id, buildJsonObject { put("slug", slug.trim().ifBlank { null }); put("published", on) })) reload() }
                                }, enabled = slug.isNotBlank())
                            }
                            OutlinedTextField(slug, { slug = it.lowercase().filter { c -> c.isLetterOrDigit() || c == '-' } }, singleLine = true, label = { Text(stringResource(R.string.site_address)) },
                                supportingText = { Text("/pages/" + slug.ifBlank { "…" }) }, modifier = Modifier.fillMaxWidth(),
                                trailingIcon = { if (slug != p.slug.orEmpty() && slug.isNotBlank()) IconButton(onClick = { state.launch { if (state.updateProject(id, buildJsonObject { put("slug", slug.trim()) })) reload() } }) { Icon(Icons.Outlined.Check, stringResource(R.string.save)) } })
                        }
                        d.public_url?.takeIf { p.published }?.let { url ->
                            val full = if (url.startsWith("http")) url else runCatching { state.api.server }.getOrNull().orEmpty() + url
                            Text(full, color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodySmall)
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                OutlinedButton(onClick = { ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(p.name, full)); Toast.makeText(ctx, copied, Toast.LENGTH_SHORT).show() }) { Text(stringResource(R.string.copy_link)) }
                                OutlinedButton(onClick = { runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(full)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) } }) { Text(stringResource(R.string.open)) }
                            }
                        }
                    }
                }
            }
            if (p.kind == "notebook") item {
                Text(stringResource(R.string.notebook_hint), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(16.dp))
            }
            item { SectionTitle(stringResource(R.string.files)) }
            items(treeOrder(d.files), key = { it.id }) { f ->
                val depth = f.path.count { it == '/' }
                ListItem(
                    headlineContent = { Text(f.path.substringAfterLast('/'), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = if (f.is_dir) null else { { Text(listOfNotNull(Formatter.formatShortFileSize(ctx, f.size), f.updated_by_name).joinToString(" · ")) } },
                    leadingContent = { Icon(if (f.is_dir) Icons.Outlined.Folder else if (f.text) Icons.Outlined.Description else mimeIcon(f.mime, f.path), null, tint = if (f.is_dir) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant) },
                    trailingContent = if (d.can_write) { { IconButton(onClick = { fileFor = f }) { Icon(Icons.Outlined.MoreVert, null) } } } else null,
                    modifier = Modifier.padding(start = (depth * 16).dp).clickable(enabled = !f.is_dir) {
                        if (f.text) onFile(f.id)
                        else { busy = "open"; state.launch { state.projectFileDownload(id, f)?.let { openFile(ctx, it, f.mime) }; busy = null } }
                    },
                )
            }
        }
    }

    when (dialog) {
        "file", "folder" -> NameDialog(stringResource(if (dialog == "file") R.string.new_file else R.string.new_folder), stringResource(R.string.file_path), "", onDismiss = { dialog = null }) { path ->
            val dir = dialog == "folder"; dialog = null
            state.launch { if (state.createProjectFile(id, path, dir)) reload() }
        }
        "rename" -> NameDialog(stringResource(R.string.rename), stringResource(R.string.label_name), p?.name.orEmpty(), onDismiss = { dialog = null }) { n ->
            dialog = null; state.launch { if (state.updateProject(id, buildJsonObject { put("name", n) })) reload() }
        }
        "delete" -> AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text(stringResource(R.string.delete_project_title, p?.name.orEmpty())) },
            text = { Text(stringResource(R.string.delete_project_message)) },
            confirmButton = { TextButton(onClick = { dialog = null; state.launch { if (state.deleteProject(id)) onBack() } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { dialog = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    fileFor?.let { f ->
        var mode by remember { mutableStateOf<String?>(null) }
        when (mode) {
            null -> AlertDialog(
                onDismissRequest = { fileFor = null },
                title = { Text(f.path, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                text = {
                    Column {
                        ListItem(headlineContent = { Text(stringResource(R.string.rename_move)) }, leadingContent = { Icon(Icons.Outlined.DriveFileRenameOutline, null) }, colors = sheetItemColors(), modifier = Modifier.clickable { mode = "move" })
                        ListItem(headlineContent = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) }, leadingContent = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) }, colors = sheetItemColors(), modifier = Modifier.clickable { mode = "delete" })
                    }
                },
                confirmButton = { TextButton(onClick = { fileFor = null }) { Text(stringResource(R.string.cancel)) } },
            )
            "move" -> NameDialog(stringResource(R.string.rename_move), stringResource(R.string.file_path), f.path, onDismiss = { fileFor = null }) { path ->
                fileFor = null; state.launch { if (state.moveProjectFile(id, f.id, path)) reload() }
            }
            "delete" -> AlertDialog(
                onDismissRequest = { fileFor = null },
                title = { Text(stringResource(R.string.delete_attachment_title, f.path)) },
                text = { if (f.is_dir) Text(stringResource(R.string.delete_drive_folder_hint)) },
                confirmButton = { TextButton(onClick = { fileFor = null; state.launch { if (state.deleteProjectFile(id, f.id)) reload() } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
                dismissButton = { TextButton(onClick = { fileFor = null }) { Text(stringResource(R.string.cancel)) } },
            )
        }
    }
}

/** Arborescence : dans chaque dossier, les sous-dossiers d'abord, puis les fichiers, par nom. */
private fun treeOrder(files: List<ProjectFile>): List<ProjectFile> {
    val byParent = files.groupBy { it.path.substringBeforeLast('/', "") }
    val out = mutableListOf<ProjectFile>()
    fun walk(dir: String) {
        byParent[dir].orEmpty().sortedWith(compareBy({ !it.is_dir }, { it.path.lowercase() })).forEach { f -> out += f; if (f.is_dir) walk(f.path) }
    }
    walk("")
    // Fichiers dont le dossier parent n'est pas listé : à la fin, pour ne rien perdre.
    return out + files.filter { it !in out }
}

/** Fichier téléchargé, proposé à une autre application (partage, enregistrement). */
fun shareFile(ctx: android.content.Context, file: java.io.File, mime: String) {
    val uri = androidx.core.content.FileProvider.getUriForFile(ctx, ctx.packageName + ".files", file)
    ctx.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType(mime).putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION), null)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
}

/** Lecture d'un fichier texte d'un projet (code, LaTeX…) ; la modification se fait dans l'éditeur. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ProjectFileScreen(state: AppState, projectId: String, fileId: String, onBack: () -> Unit) {
    val ctx = LocalContext.current
    var text by remember { mutableStateOf<String?>(null) }
    var name by remember { mutableStateOf("") }
    LaunchedEffect(fileId) {
        name = state.project(projectId)?.files?.firstOrNull { it.id == fileId }?.path.orEmpty()
        text = state.projectFileText(projectId, fileId) ?: ""
    }
    Scaffold(topBar = {
        TopAppBar(
            title = { Text(name.substringAfterLast('/'), maxLines = 1, overflow = TextOverflow.Ellipsis) },
            navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            actions = { IconButton(onClick = { state.ws?.let { openWeb(ctx, "/w/$it/projects/$projectId") } }) { Icon(Icons.Outlined.Edit, stringResource(R.string.edit_in_editor)) } },
        )
    }) { padding ->
        val t = text
        if (t == null) { Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }; return@Scaffold }
        SelectionContainer {
            val lines = t.lines()
            val width = lines.size.toString().length
            Text(
                lines.mapIndexed { i, l -> (i + 1).toString().padStart(width) + "  " + l }.joinToString("\n"),
                fontFamily = FontFamily.Monospace, fontSize = 12.sp, softWrap = false,
                modifier = Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).horizontalScroll(rememberScrollState()).padding(12.dp),
            )
        }
    }
}
