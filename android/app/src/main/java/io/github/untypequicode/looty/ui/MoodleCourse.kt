package io.github.untypequicode.looty.ui

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.MoodleLoginActivity
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.MoodleAssignment
import io.github.untypequicode.looty.api.MoodleCourseContent
import io.github.untypequicode.looty.api.MoodleFile
import io.github.untypequicode.looty.api.MoodleModule
import kotlinx.coroutines.launch

/** Ouvre une page du site Moodle dans l'application, déjà connecté si possible. */
fun openMoodleSite(state: AppState, ctx: android.content.Context, url: String?) = state.launch {
    state.moodleOpen(url)?.let { MoodleLoginActivity.browse(ctx, it) }
}

/** Activités lues dans l'application ; les autres (tests, forums…) s'ouvrent sur le site. */
private val INLINE = setOf("page", "book", "folder", "assign", "resource", "label")

private fun moduleIcon(type: String): ImageVector = when (type) {
    "resource" -> Icons.Outlined.Description
    "page", "book" -> Icons.Outlined.MenuBook
    "folder" -> Icons.Outlined.Folder
    "url" -> Icons.Outlined.Link
    "assign" -> Icons.Outlined.Assignment
    "quiz" -> Icons.Outlined.Quiz
    "forum" -> Icons.Outlined.Forum
    else -> Icons.Outlined.Widgets
}

/** Un cours Moodle : sections et activités, fichiers ouverts sur le téléphone, pages et devoirs lus ici. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MoodleCourseScreen(state: AppState, id: Long, name: String, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var course by remember { mutableStateOf<MoodleCourseContent?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var openModule by remember { mutableStateOf<MoodleModule?>(null) }
    var downloading by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(id) { state.moodleCourse(id).onSuccess { course = it }.onFailure { error = errorText(ctx, it) } }

    fun openMoodleFile(f: MoodleFile) {
        val path = f.url ?: return
        downloading = f.name
        scope.launch {
            state.moodleFile(path, f.name).onSuccess { openFile(ctx, it, f.mime) }.onFailure { error = errorText(ctx, it) }
            downloading = null
        }
    }
    fun tap(m: MoodleModule) {
        when {
            m.type == "label" -> {}
            m.type == "url" && m.link != null -> runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(m.link)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
            m.type == "resource" && m.files.size == 1 -> openMoodleFile(m.files[0])
            m.type in INLINE -> openModule = m
            else -> openMoodleSite(state, ctx, m.url)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = { IconButton(onClick = { openMoodleSite(state, ctx, course?.url) }) { Icon(Icons.Outlined.OpenInBrowser, stringResource(R.string.moodle_open_on_site)) } },
            )
        },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            if (downloading != null) LinearProgressIndicator(Modifier.fillMaxWidth())
            when {
                error != null && course == null -> EmptyState(Icons.Outlined.CloudOff, error!!)
                course == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                course!!.sections.isEmpty() -> EmptyState(Icons.Outlined.School, stringResource(R.string.moodle_course_empty))
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
                    course!!.sections.forEach { s ->
                        item(key = "s${s.id}") {
                            Column(Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp)) {
                                s.name?.takeIf { it.isNotBlank() }?.let { Text(it, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary) }
                                if (s.summary.isNotBlank()) HtmlBlocks(state, s.summary, Modifier.padding(top = 4.dp))
                            }
                        }
                        items(s.modules, key = { "m${it.id}" }) { m ->
                            if (m.type == "label") {
                                HtmlBlocks(state, m.description, Modifier.padding(horizontal = 16.dp, vertical = 6.dp))
                            } else {
                                ListItem(
                                    headlineContent = { Text(m.name, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                                    supportingContent = {
                                        val date = m.dates.lastOrNull()?.let { d -> d.time?.let { listOfNotNull(d.label, mailFullDate(it)).joinToString(" ") } }
                                        Text(listOfNotNull(if (m.type in INLINE || m.type == "url") m.type_name else stringResource(R.string.moodle_on_site), date).joinToString(" · "), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                    },
                                    leadingContent = { Icon(moduleIcon(m.type), null, tint = MaterialTheme.colorScheme.primary) },
                                    trailingContent = when (m.done) {
                                        true -> { { Icon(Icons.Outlined.CheckCircle, null, tint = MaterialTheme.colorScheme.primary) } }
                                        false -> { { Icon(Icons.Outlined.RadioButtonUnchecked, null, tint = MaterialTheme.colorScheme.onSurfaceVariant) } }
                                        null -> if (m.type in INLINE) null else { { Icon(Icons.Outlined.OpenInNew, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant) } }
                                    },
                                    modifier = Modifier.clickable { tap(m) },
                                )
                            }
                        }
                    }
                }
            }
        }
    }
    openModule?.let { m -> ModuleSheet(state, id, m, onFile = ::openMoodleFile, onDismiss = { openModule = null }) }
}

/** Activité ouverte : page ou livre lu ici, dossier, devoir (consigne, état, note). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ModuleSheet(state: AppState, courseId: Long, m: MoodleModule, onFile: (MoodleFile) -> Unit, onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    val pages = remember { mutableStateMapOf<String, String>() }
    var assign by remember { mutableStateOf<MoodleAssignment?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(m.id) {
        m.files.filter { it.html }.forEach { f -> state.moodlePage(f.source).onSuccess { pages[f.source] = it }.onFailure { error = errorText(ctx, it) } }
        if (m.type == "assign") state.moodleAssignment(courseId, m.id).onSuccess { assign = it }.onFailure { error = errorText(ctx, it) }
    }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        LazyColumn(Modifier.fillMaxWidth().navigationBarsPadding(), contentPadding = PaddingValues(start = 20.dp, end = 20.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            item { Text(m.name, style = MaterialTheme.typography.titleLarge) }
            if (m.description.isNotBlank()) item { HtmlBlocks(state, m.description) }
            error?.let { e -> item { Text(e, color = MaterialTheme.colorScheme.error) } }
            m.files.filter { it.html }.forEach { f ->
                item(key = f.source) {
                    val html = pages[f.source]
                    if (html == null) LinearProgressIndicator(Modifier.fillMaxWidth()) else HtmlBlocks(state, html)
                }
            }
            if (m.type == "assign") item {
                val a = assign
                if (a == null) LinearProgressIndicator(Modifier.fillMaxWidth()) else Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (a.intro.isNotBlank()) HtmlBlocks(state, a.intro)
                    @Composable fun row(label: Int, value: String, strong: Boolean = false) = Row {
                        Text(stringResource(label), Modifier.width(110.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(value, fontWeight = if (strong) FontWeight.SemiBold else null)
                    }
                    a.due?.let { row(R.string.moodle_due, mailFullDate(it)) }
                    a.cutoff?.let { row(R.string.moodle_cutoff, mailFullDate(it)) }
                    a.status?.let { s ->
                        row(R.string.moodle_submission, stringResource(when (s) {
                            "submitted" -> R.string.moodle_status_submitted
                            "draft" -> R.string.moodle_status_draft
                            "reopened" -> R.string.moodle_status_reopened
                            else -> R.string.moodle_status_new
                        }), strong = true)
                    }
                    a.grading?.let { g -> row(R.string.moodle_grading, stringResource(if (g == "graded") R.string.moodle_graded else R.string.moodle_notgraded)) }
                    a.grade?.let { row(R.string.moodle_grade, it, strong = true) }
                    if (a.attachments.isNotEmpty()) Text(stringResource(R.string.moodle_attachments), style = MaterialTheme.typography.labelLarge)
                    a.attachments.forEach { f ->
                        FileRow(f.name, f.size) { onFile(MoodleFile(name = f.name, size = f.size, mime = f.mime, url = f.url)) }
                    }
                    Button(onClick = { openMoodleSite(state, ctx, a.url) }) { Icon(Icons.Outlined.OpenInBrowser, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.moodle_submit)) }
                }
            }
            val files = if (m.type == "page" || m.type == "book") emptyList() else m.files.filter { !it.html }
            items(files, key = { "f" + it.source }) { f -> FileRow((if (f.path != "/") f.path.drop(1) else "") + f.name, f.size) { onFile(f) } }
        }
    }
}

@Composable
private fun FileRow(name: String, size: Long, onClick: () -> Unit) {
    Surface(shape = RoundedCornerShape(10.dp), tonalElevation = 2.dp, modifier = Modifier.fillMaxWidth().clickable(onClick = onClick)) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.InsertDriveFile, null, tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.width(10.dp))
            Text(name, Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(android.text.format.Formatter.formatShortFileSize(LocalContext.current, size), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
