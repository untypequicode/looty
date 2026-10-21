package io.github.untypequicode.looty.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.*
import kotlinx.coroutines.launch
import java.text.Collator
import java.time.Instant

/** Couleurs proposées pour un cours (les mêmes que sur le web). */
private val COURSE_COLORS = listOf("#e5484d", "#e0701a", "#d9a400", "#3da35d", "#1ba6a6", "#2a78d6", "#6e56cf", "#c2409a", "#6b7280")

private fun parseColor(hex: String?): Color? = hex?.let { runCatching { Color(android.graphics.Color.parseColor(it)) }.getOrNull() }

/** Cours Moodle de l'utilisateur et ce que Looty y ajoute (GLOSSARY) : dossiers, personnalisation, ordre. */
class CourseBook(val courses: List<MoodleCourse>, val prefs: CoursePrefs) {
    val folders get() = prefs.folders
    fun custom(id: Long): CourseCustom = prefs.custom[id.toString()] ?: CourseCustom()
    fun folderOf(id: Long): String? = prefs.assign[id.toString()]?.takeIf { f -> folders.any { it.id == f } }
    fun childrenOf(parent: String?) = folders.filter { it.parent == parent }
    fun nameOf(c: MoodleCourse): String = custom(c.id).short?.takeIf { it.isNotBlank() } ?: c.fullname ?: c.shortname.orEmpty()
    fun byId(id: Long?) = courses.firstOrNull { it.id == id }
    /** Le dossier et tous ses sous-dossiers. */
    fun treeOf(id: String): List<String> = listOf(id) + childrenOf(id).flatMap { treeOf(it.id) }
    /** Chemin lisible : « Licence / Semestre 3 ». */
    fun pathOf(id: String): String {
        val f = folders.firstOrNull { it.id == id } ?: return ""
        return f.parent?.let { pathOf(it) + " / " + f.name } ?: f.name
    }

    fun withFolder(name: String, parent: String?): Pair<CoursePrefs, CourseFolder> {
        val f = CourseFolder(java.util.UUID.randomUUID().toString().take(8), name, parent)
        return prefs.copy(folders = folders + f) to f
    }
    fun renamed(id: String, name: String) = prefs.copy(folders = folders.map { if (it.id == id) it.copy(name = name) else it })
    /** Supprime le dossier et ses sous-dossiers ; leurs cours n'ont plus de dossier. */
    fun withoutFolder(id: String): CoursePrefs {
        val gone = treeOf(id).toSet()
        return prefs.copy(folders = folders.filter { it.id !in gone }, assign = prefs.assign.filterValues { it !in gone })
    }
    fun moved(course: Long, folder: String?) =
        prefs.copy(assign = if (folder == null) prefs.assign - course.toString() else prefs.assign + (course.toString() to folder))
    fun customized(course: Long, value: CourseCustom): CoursePrefs {
        val clean = CourseCustom(value.short?.trim()?.ifBlank { null }, value.color?.ifBlank { null }, value.icon?.trim()?.ifBlank { null })
        val empty = clean.short == null && clean.color == null && clean.icon == null
        return prefs.copy(custom = if (empty) prefs.custom - course.toString() else prefs.custom + (course.toString() to clean))
    }
}

/** Pastille d'un cours : trois lettres (ou l'icône choisie) sur la couleur choisie, sinon une couleur tirée de son numéro. */
@Composable
fun CourseBadge(c: MoodleCourse, custom: CourseCustom, size: Int = 40) {
    val base = parseColor(custom.color) ?: Color.hsv(((c.id * 67) % 360).toFloat(), 0.55f, 0.75f)
    val label = custom.icon?.takeIf { it.isNotBlank() } ?: (custom.short ?: c.shortname ?: c.fullname).orEmpty().take(3).uppercase()
    Box(Modifier.size(size.dp).clip(RoundedCornerShape(10.dp)).background(base.copy(alpha = 0.2f)), contentAlignment = Alignment.Center) {
        Text(label, color = base, fontWeight = FontWeight.Bold, fontSize = (size * if (custom.icon.isNullOrBlank()) 0.3 else 0.5).sp, maxLines = 1)
    }
}

/** Une ligne de cours : pastille, nom, catégorie, progression ; le menu ⋮ ouvre les actions. */
@Composable
fun CourseRow(c: MoodleCourse, book: CourseBook, onClick: () -> Unit, onMenu: (() -> Unit)?, compact: Boolean = false) {
    val custom = book.custom(c.id)
    ListItem(
        headlineContent = { Text(book.nameOf(c), maxLines = if (compact) 1 else 2, overflow = TextOverflow.Ellipsis) },
        supportingContent = if (compact) null else ({
            Column {
                Text(listOfNotNull(c.shortname, c.category).joinToString(" · "), maxLines = 1, overflow = TextOverflow.Ellipsis)
                c.progress?.let { p -> LinearProgressIndicator({ (p / 100).toFloat().coerceIn(0f, 1f) }, Modifier.fillMaxWidth().padding(top = 6.dp)) }
            }
        }),
        leadingContent = { CourseBadge(c, custom, if (compact) 32 else 40) },
        trailingContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (c.favourite) Icon(Icons.Outlined.Star, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
                if (c.hidden) Icon(Icons.Outlined.VisibilityOff, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                if (onMenu != null) IconButton(onClick = onMenu) { Icon(Icons.Outlined.MoreVert, stringResource(R.string.crs_actions)) }
            }
        },
        modifier = Modifier.clickable(onClick = onClick),
    )
}

private sealed interface CRow {
    data class Header(val key: String, val label: String, val depth: Int, val folder: CourseFolder?, val count: Int) : CRow
    data class Item(val course: MoodleCourse, val depth: Int) : CRow
    data class Hint(val text: Int, val depth: Int) : CRow
}

private fun inProgress(c: MoodleCourse, now: Long) = !c.hidden && (c.startdate == null || c.startdate <= now) && !((c.enddate ?: Long.MAX_VALUE) < now || c.progress == 100.0)

internal fun matches(c: MoodleCourse, filter: String, now: Long): Boolean = when (filter) {
    "progress" -> inProgress(c, now)
    "upcoming" -> !c.hidden && c.startdate != null && c.startdate > now
    "past" -> !c.hidden && ((c.enddate != null && c.enddate < now) || c.progress == 100.0)
    "favourites" -> !c.hidden && c.favourite
    "hidden" -> c.hidden
    else -> !c.hidden
}

/** Prochaine échéance de chaque cours (les échéances à venir d'abord, puis les retards). */
private fun nextDue(deadlines: List<MoodleDeadline>): Map<Long, Long> {
    val m = HashMap<Long, Long>()
    for (d in deadlines) {
        val id = d.course_id ?: continue
        val ts = d.due?.let { runCatching { Instant.parse(it).toEpochMilli() }.getOrNull() } ?: continue
        val rank = if (d.overdue) ts + 1_000_000_000_000_000L else ts
        if (rank < (m[id] ?: Long.MAX_VALUE)) m[id] = rank
    }
    return m
}

/** Onglet « Cours » : recherche, tri, filtre, regroupement par catégorie ou par dossiers, actions de chaque cours. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CoursesTab(state: AppState, onCourse: (Long, String) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var courses by remember { mutableStateOf<List<MoodleCourse>?>(null) }
    var deadlines by remember { mutableStateOf<List<MoodleDeadline>>(emptyList()) }
    var error by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(false) }
    fun reload() {
        scope.launch {
            loading = true
            state.loadCoursePrefs()
            state.moodleDeadlines().onSuccess { deadlines = it }
            state.moodleCourses().onSuccess { courses = it; error = null }.onFailure { error = errorText(ctx, it) }
            loading = false
        }
    }
    LaunchedEffect(Unit) { reload() }

    // Les réglages de départ sont ceux du premier panneau « Mes cours » du tableau de bord.
    val panel = state.boardPanels?.firstOrNull { it.kind == "courses" }?.opts
    var sort by rememberSaveable { mutableStateOf(panel?.sort ?: "access") }
    var filter by rememberSaveable { mutableStateOf(panel?.filter ?: "progress") }
    var group by rememberSaveable { mutableStateOf(panel?.group ?: "category") }
    var query by rememberSaveable { mutableStateOf("") }
    var options by remember { mutableStateOf(false) }
    var collapsed by remember { mutableStateOf(setOf<String>()) }
    var actions by remember { mutableStateOf<MoodleCourse?>(null) }
    var moving by remember { mutableStateOf<MoodleCourse?>(null) }
    var customizing by remember { mutableStateOf<MoodleCourse?>(null) }
    var folderMenu by remember { mutableStateOf<CourseFolder?>(null) }
    var ask by remember { mutableStateOf<Ask?>(null) }
    var confirmDelete by remember { mutableStateOf<CourseFolder?>(null) }

    val list = courses
    val book = remember(list, state.coursePrefs) { CourseBook(list.orEmpty(), state.coursePrefs) }
    val now = System.currentTimeMillis() / 1000
    val collator = remember { Collator.getInstance().apply { strength = Collator.PRIMARY } }
    val due = remember(deadlines) { nextDue(deadlines) }

    val shown = remember(book, filter, sort, query, due) {
        val q = query.trim().lowercase()
        val byName = Comparator<MoodleCourse> { a, b -> collator.compare(book.nameOf(a), book.nameOf(b)) }
        val manual = book.prefs.order.filter { book.byId(it) != null } + book.courses.filter { it.id !in book.prefs.order }.sortedWith(byName).map { it.id }
        val cmp: Comparator<MoodleCourse> = when (sort) {
            "name" -> byName
            "progress" -> compareBy<MoodleCourse> { it.progress ?: Double.MAX_VALUE }.then(byName)
            "deadline" -> compareBy<MoodleCourse> { due[it.id] ?: Long.MAX_VALUE }.then(byName)
            "manual" -> compareBy { manual.indexOf(it.id) }
            else -> compareByDescending<MoodleCourse> { it.lastaccess ?: 0L }.then(byName)
        }
        book.courses.filter { matches(it, filter, now) }
            .filter { q.isEmpty() || listOfNotNull(it.fullname, it.shortname, book.nameOf(it), it.category).any { s -> s.lowercase().contains(q) } }
            .sortedWith(cmp)
    }

    val noCategory = stringResource(R.string.crs_no_category)
    val noFolder = stringResource(R.string.crs_no_folder)
    val rows = remember(shown, group, book, collapsed, query) {
        val out = ArrayList<CRow>()
        when (group) {
            "category" -> {
                val groups = shown.groupBy { it.category.orEmpty() }
                groups.entries.sortedWith { a, b -> if (a.key.isNotEmpty() && b.key.isEmpty()) -1 else if (a.key.isEmpty() && b.key.isNotEmpty()) 1 else collator.compare(a.key, b.key) }.forEach { (k, cs) ->
                    val key = "cat:$k"
                    if (groups.size > 1 || k.isNotEmpty()) out += CRow.Header(key, k.ifEmpty { noCategory }, 0, null, cs.size)
                    if (key !in collapsed) cs.forEach { out += CRow.Item(it, 0) }
                }
            }
            "folders" -> {
                fun inFolder(id: String?) = shown.filter { book.folderOf(it.id) == id }
                val loose = inFolder(null)
                if (loose.isNotEmpty() || book.folders.isEmpty()) {
                    if (book.folders.isNotEmpty()) out += CRow.Header("f:", noFolder, 0, null, loose.size)
                    if ("f:" !in collapsed) loose.forEach { out += CRow.Item(it, 0) }
                }
                fun walk(parent: String?, depth: Int) {
                    for (f in book.childrenOf(parent).sortedWith { a, b -> collator.compare(a.name, b.name) }) {
                        val cs = inFolder(f.id)
                        val before = out.size
                        out += CRow.Header("f:${f.id}", f.name, depth, f, cs.size)
                        if ("f:${f.id}" !in collapsed) {
                            cs.forEach { out += CRow.Item(it, depth) }
                            if (cs.isEmpty() && book.childrenOf(f.id).isEmpty() && query.isBlank()) out += CRow.Hint(R.string.crs_empty_folder, depth)
                            walk(f.id, depth + 1)
                        }
                        // Pendant une recherche, les dossiers sans résultat disparaissent.
                        if (query.isNotBlank() && cs.isEmpty() && out.size == before + 1) out.removeAt(before)
                    }
                }
                walk(null, 0)
            }
            else -> shown.forEach { out += CRow.Item(it, 0) }
        }
        out
    }

    Column(Modifier.fillMaxSize()) {
        Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(query, { query = it }, singleLine = true, modifier = Modifier.weight(1f), placeholder = { Text(stringResource(R.string.crs_search)) },
                leadingIcon = { Icon(Icons.Outlined.Search, null) },
                trailingIcon = { if (query.isNotEmpty()) IconButton(onClick = { query = "" }) { Icon(Icons.Outlined.Close, null) } })
            IconButton(onClick = { options = true }) { Icon(Icons.Outlined.Tune, stringResource(R.string.crs_options)) }
            if (group == "folders") IconButton(onClick = { ask = Ask(R.string.crs_new_folder, "") { name -> state.launch { state.saveCoursePrefs(book.withFolder(name, null).first) } } }) {
                Icon(Icons.Outlined.CreateNewFolder, stringResource(R.string.crs_new_folder))
            }
        }
        PullToRefreshBox(isRefreshing = loading && list != null, onRefresh = { reload() }, modifier = Modifier.fillMaxSize()) {
            when {
                list == null && error != null -> EmptyState(Icons.Outlined.CloudOff, error!!)
                list == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
                    if (shown.isEmpty() && (group != "folders" || book.folders.isEmpty())) item {
                        EmptyState(Icons.Outlined.School, if (query.isNotBlank()) stringResource(R.string.crs_no_match) else stringResource(emptyText(filter)))
                    }
                    items(rows, key = { r -> when (r) { is CRow.Header -> r.key; is CRow.Item -> "c${r.course.id}"; is CRow.Hint -> "h${r.depth}${r.text}" } }) { r ->
                        when (r) {
                            is CRow.Header -> Row(Modifier.fillMaxWidth().clickable { collapsed = if (r.key in collapsed) collapsed - r.key else collapsed + r.key }
                                .padding(start = (16 + 16 * r.depth).dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                                Icon(if (r.key in collapsed) Icons.Outlined.ChevronRight else Icons.Outlined.ExpandMore, null, Modifier.size(20.dp))
                                Icon(if (group == "folders") Icons.Outlined.Folder else Icons.Outlined.Category, null, Modifier.padding(start = 6.dp).size(18.dp), tint = MaterialTheme.colorScheme.primary)
                                Text("${r.label}  ·  ${r.count}", Modifier.weight(1f).padding(horizontal = 8.dp, vertical = 12.dp), style = MaterialTheme.typography.titleSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                if (r.folder != null) IconButton(onClick = { folderMenu = r.folder }) { Icon(Icons.Outlined.MoreVert, stringResource(R.string.crs_actions)) }
                            }
                            is CRow.Item -> Box(Modifier.padding(start = (16 * r.depth).dp)) {
                                CourseRow(r.course, book, onClick = { onCourse(r.course.id, book.nameOf(r.course)) }, onMenu = { actions = r.course })
                            }
                            is CRow.Hint -> Text(stringResource(r.text), Modifier.padding(start = (56 + 16 * r.depth).dp, end = 16.dp, bottom = 8.dp), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
    }

    if (options) ModalBottomSheet(onDismissRequest = { options = false }) {
        NoNavScrim()
        Column(Modifier.padding(bottom = 24.dp)) {
            OptionGroup(R.string.crs_sort_by, listOf("access" to R.string.crs_sort_access, "name" to R.string.crs_sort_name, "progress" to R.string.crs_sort_progress, "deadline" to R.string.crs_sort_deadline, "manual" to R.string.crs_sort_manual), sort) { sort = it }
            OptionGroup(R.string.crs_show, listOf("progress" to R.string.crs_f_progress, "upcoming" to R.string.crs_f_upcoming, "past" to R.string.crs_f_past, "favourites" to R.string.crs_f_favourites, "hidden" to R.string.crs_f_hidden, "all" to R.string.crs_f_all), filter) { filter = it }
            OptionGroup(R.string.crs_group_by, listOf("category" to R.string.crs_g_category, "folders" to R.string.crs_g_folders, "none" to R.string.crs_g_none), group) { group = it }
        }
    }

    actions?.let { c ->
        ModalBottomSheet(onDismissRequest = { actions = null }) {
            NoNavScrim()
            Column(Modifier.padding(bottom = 24.dp)) {
                Text(book.nameOf(c), Modifier.padding(horizontal = 24.dp, vertical = 8.dp), style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
                SheetAction(if (c.favourite) Icons.Outlined.StarBorder else Icons.Outlined.Star, if (c.favourite) R.string.crs_unstar else R.string.crs_star) {
                    actions = null; state.launch { if (state.moodleSetCourseFlag(c.id, "favourite", !c.favourite)) courses = courses?.map { if (it.id == c.id) it.copy(favourite = !c.favourite) else it } }
                }
                SheetAction(if (c.hidden) Icons.Outlined.Visibility else Icons.Outlined.VisibilityOff, if (c.hidden) R.string.crs_show_course else R.string.crs_hide) {
                    actions = null; state.launch { if (state.moodleSetCourseFlag(c.id, "hidden", !c.hidden)) courses = courses?.map { if (it.id == c.id) it.copy(hidden = !c.hidden) else it } }
                }
                SheetAction(Icons.Outlined.Palette, R.string.crs_customize) { actions = null; customizing = c }
                SheetAction(Icons.Outlined.DriveFileMove, R.string.crs_move_to) { actions = null; moving = c }
            }
        }
    }

    moving?.let { c ->
        ModalBottomSheet(onDismissRequest = { moving = null }) {
            NoNavScrim()
            val current = book.folderOf(c.id)
            LazyColumn(Modifier.padding(bottom = 24.dp)) {
                item { Text(stringResource(R.string.crs_move_to), Modifier.padding(horizontal = 24.dp, vertical = 8.dp), style = MaterialTheme.typography.titleMedium) }
                if (current != null) item { SheetAction(Icons.Outlined.Close, R.string.crs_no_folder) { moving = null; state.launch { state.saveCoursePrefs(book.moved(c.id, null)) } } }
                items(book.folders.sortedWith { a, b -> collator.compare(book.pathOf(a.id), book.pathOf(b.id)) }, key = { it.id }) { f ->
                    ListItem(
                        headlineContent = { Text(book.pathOf(f.id), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        leadingContent = { Icon(Icons.Outlined.Folder, null) },
                        trailingContent = { if (f.id == current) Icon(Icons.Outlined.Check, null) },
                        colors = sheetItemColors(),
                        modifier = Modifier.clickable { moving = null; state.launch { state.saveCoursePrefs(book.moved(c.id, f.id)) } },
                    )
                }
                item {
                    SheetAction(Icons.Outlined.CreateNewFolder, R.string.crs_new_folder_here) {
                        moving = null
                        ask = Ask(R.string.crs_new_folder, "") { name -> state.launch { val (p, f) = book.withFolder(name, null); state.saveCoursePrefs(CourseBook(book.courses, p).moved(c.id, f.id)) } }
                    }
                }
            }
        }
    }

    folderMenu?.let { f ->
        ModalBottomSheet(onDismissRequest = { folderMenu = null }) {
            NoNavScrim()
            Column(Modifier.padding(bottom = 24.dp)) {
                Text(book.pathOf(f.id), Modifier.padding(horizontal = 24.dp, vertical = 8.dp), style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                SheetAction(Icons.Outlined.CreateNewFolder, R.string.crs_new_subfolder) { folderMenu = null; ask = Ask(R.string.crs_new_subfolder, "") { name -> state.launch { state.saveCoursePrefs(book.withFolder(name, f.id).first) } } }
                SheetAction(Icons.Outlined.Edit, R.string.crs_rename_folder) { folderMenu = null; ask = Ask(R.string.crs_rename_folder, f.name) { name -> state.launch { state.saveCoursePrefs(book.renamed(f.id, name)) } } }
                SheetAction(Icons.Outlined.Delete, R.string.crs_delete_folder, danger = true) { folderMenu = null; confirmDelete = f }
            }
        }
    }

    ask?.let { a ->
        var text by remember(a) { mutableStateOf(a.initial) }
        AlertDialog(
            onDismissRequest = { ask = null },
            title = { Text(stringResource(a.title)) },
            text = { OutlinedTextField(text, { text = it }, singleLine = true, label = { Text(stringResource(R.string.crs_folder_name)) }) },
            confirmButton = { TextButton(enabled = text.isNotBlank(), onClick = { ask = null; a.done(text.trim()) }) { Text(stringResource(R.string.ok)) } },
            dismissButton = { TextButton(onClick = { ask = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }

    confirmDelete?.let { f ->
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text(stringResource(R.string.crs_delete_folder_title, f.name)) },
            text = { Text(stringResource(R.string.crs_delete_folder_hint)) },
            confirmButton = { TextButton(onClick = { confirmDelete = null; state.launch { state.saveCoursePrefs(book.withoutFolder(f.id)) } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { confirmDelete = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }

    customizing?.let { c ->
        CustomizeDialog(c, book.custom(c.id), onDismiss = { customizing = null }) { v -> customizing = null; state.launch { state.saveCoursePrefs(book.customized(c.id, v)) } }
    }
}

private class Ask(val title: Int, val initial: String, val done: (String) -> Unit)

private fun emptyText(filter: String) = when (filter) {
    "progress" -> R.string.crs_empty_progress
    "upcoming" -> R.string.crs_empty_upcoming
    "past" -> R.string.crs_empty_past
    "favourites" -> R.string.crs_empty_favourites
    "hidden" -> R.string.crs_empty_hidden
    else -> R.string.moodle_no_courses
}

@Composable
private fun OptionGroup(title: Int, options: List<Pair<String, Int>>, selected: String, onPick: (String) -> Unit) {
    Text(stringResource(title), Modifier.padding(start = 24.dp, top = 12.dp, bottom = 4.dp), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
    options.forEach { (value, label) ->
        ListItem(
            headlineContent = { Text(stringResource(label)) },
            trailingContent = { if (value == selected) Icon(Icons.Outlined.Check, null) },
            colors = sheetItemColors(),
            modifier = Modifier.clickable { onPick(value) },
        )
    }
}

@Composable
private fun SheetAction(icon: androidx.compose.ui.graphics.vector.ImageVector, label: Int, danger: Boolean = false, onClick: () -> Unit) {
    val tint = if (danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant
    ListItem(
        headlineContent = { Text(stringResource(label), color = if (danger) MaterialTheme.colorScheme.error else Color.Unspecified) },
        leadingContent = { Icon(icon, null, tint = tint) },
        colors = sheetItemColors(),
        modifier = Modifier.clickable(onClick = onClick),
    )
}

/** Personnalisation d'un cours (GLOSSARY) : nom court, couleur et icône, pour ce membre seulement. */
@Composable
private fun CustomizeDialog(c: MoodleCourse, current: CourseCustom, onDismiss: () -> Unit, onSave: (CourseCustom) -> Unit) {
    var short by remember { mutableStateOf(current.short.orEmpty()) }
    var color by remember { mutableStateOf(current.color.orEmpty()) }
    var icon by remember { mutableStateOf(current.icon.orEmpty()) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.crs_customize)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(c.fullname.orEmpty(), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 2, overflow = TextOverflow.Ellipsis)
                OutlinedTextField(short, { short = it.take(40) }, singleLine = true, label = { Text(stringResource(R.string.crs_short_name)) }, placeholder = { Text(c.shortname ?: c.fullname.orEmpty()) })
                Text(stringResource(R.string.crs_color), style = MaterialTheme.typography.labelLarge)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
                    (listOf("") + COURSE_COLORS).forEach { hex ->
                        val col = parseColor(hex)
                        Box(
                            Modifier.size(24.dp).clip(CircleShape).background(col ?: MaterialTheme.colorScheme.surfaceContainerHighest).clickable { color = hex },
                            contentAlignment = Alignment.Center,
                        ) { if (color == hex) Icon(Icons.Outlined.Check, null, Modifier.size(16.dp), tint = if (col == null) MaterialTheme.colorScheme.onSurface else Color.White) }
                    }
                }
                OutlinedTextField(icon, { icon = it.take(4) }, singleLine = true, label = { Text(stringResource(R.string.crs_icon)) }, placeholder = { Text(stringResource(R.string.crs_icon_hint)) })
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    CourseBadge(c, CourseCustom(short.ifBlank { null }, color.ifBlank { null }, icon.ifBlank { null }))
                    Text(stringResource(R.string.crs_customize_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        },
        confirmButton = { TextButton(onClick = { onSave(CourseCustom(short, color, icon)) }) { Text(stringResource(R.string.save)) } },
        dismissButton = {
            Row {
                TextButton(onClick = { onSave(CourseCustom()) }) { Text(stringResource(R.string.crs_reset)) }
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
            }
        },
    )
}
