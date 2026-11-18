package io.github.untypequicode.looty.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.Session
import io.github.untypequicode.looty.api.*
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

private fun kindLabel(kind: String): Int = when (kind) {
    StudyKind.QCM -> R.string.study_kind_qcm
    StudyKind.QUESTIONNAIRE -> R.string.study_kind_questionnaire
    StudyKind.FLASHCARDS -> R.string.study_kind_flashcards
    else -> R.string.study_kind_summary
}

private fun kindIcon(kind: String) = when (kind) {
    StudyKind.QCM -> Icons.Outlined.Checklist
    StudyKind.QUESTIONNAIRE -> Icons.Outlined.QuestionAnswer
    StudyKind.FLASHCARDS -> Icons.Outlined.Style
    else -> Icons.Outlined.Description
}

/** Mes entraînements (espace courant en premier, puis les autres espaces dont l'espace personnel). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StudyListScreen(state: AppState, onBack: () -> Unit, onOpen: (String) -> Unit) {
    val s = state.session as? Session.Ready
    LaunchedEffect(Unit) { state.loadStudySets() }
    val names = s?.workspaces.orEmpty().associate { it.id to it.name }
    val sets = state.studySets.data
    Scaffold(
        topBar = { TopAppBar(title = { Text(stringResource(R.string.study_title)) }, navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } }) },
    ) { padding ->
        PullToRefreshBox(isRefreshing = state.studySets.loading && sets != null, onRefresh = { state.loadStudySets() }, modifier = Modifier.padding(padding).fillMaxSize()) {
            when {
                sets == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    if (state.studySets.error != null) Text(stringResource(R.string.error_generic)) else LoadingIndicatorCompat()
                }
                sets.isEmpty() -> EmptyState(Icons.Outlined.Checklist, stringResource(R.string.study_empty))
                else -> {
                    val groups = sets.groupBy { it.workspace_id }.entries.sortedBy { if (it.key == s?.workspaceId) 0 else 1 }
                    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
                        groups.forEach { (ws, list) ->
                            if (groups.size > 1) item(key = "h$ws") { SectionTitle(names[ws].orEmpty()) }
                            items(list, key = { it.id }) { set ->
                                ListItem(
                                    headlineContent = { Text(set.title.ifBlank { stringResource(R.string.untitled) }) },
                                    supportingContent = {
                                        val score = if (set.last_score != null && set.last_max != null) " · " + stringResource(R.string.study_last_score, set.last_score, set.last_max) else ""
                                        Text(stringResource(kindLabel(set.kind)) + " · " + pluralStringResource(R.plurals.study_items, set.item_count, set.item_count) + score)
                                    },
                                    leadingContent = { Icon(kindIcon(set.kind), null) },
                                    trailingContent = { if (set.private_to != null) Icon(Icons.Outlined.Lock, stringResource(R.string.study_personal), Modifier.size(16.dp)) },
                                    modifier = Modifier.clickable { onOpen(set.id) },
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/** Un entraînement : réviser (selon son type), historique des tentatives, liens. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StudyScreen(state: AppState, id: String, onBack: () -> Unit) {
    var set by remember(id) { mutableStateOf<StudySet?>(null) }
    var loaded by remember(id) { mutableStateOf(false) }
    var tab by remember(id) { mutableIntStateOf(0) }
    var links by remember(id) { mutableStateOf<List<StudyLink>>(emptyList()) }
    LaunchedEffect(id) {
        set = state.studySet(id); loaded = true
        links = state.studyLinks(id)
    }
    val cur = set
    Scaffold(
        topBar = { TopAppBar(title = { Text(cur?.title?.ifBlank { null } ?: stringResource(R.string.study_title), maxLines = 1) }, navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } }) },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            if (cur == null) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    if (!loaded) LoadingIndicatorCompat() else Text(stringResource(R.string.study_not_found), Modifier.padding(24.dp))
                }
            } else {
                val scored = !StudyKind.isCards(cur.kind)
                if (scored) TabRow(selectedTabIndex = tab) {
                    Tab(selected = tab == 0, onClick = { tab = 0 }, text = { Text(stringResource(R.string.study_tab_practice)) })
                    Tab(selected = tab == 1, onClick = { tab = 1 }, text = { Text(stringResource(R.string.study_tab_history)) })
                }
                Text(stringResource(R.string.study_privacy_note), Modifier.padding(horizontal = 16.dp, vertical = 6.dp), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                Box(Modifier.weight(1f)) {
                    when {
                        cur.items.isEmpty() -> Text(stringResource(R.string.study_no_items), Modifier.padding(24.dp))
                        scored && tab == 1 -> StudyHistory(state, cur, links)
                        cur.kind == StudyKind.SUMMARY -> StudySummaryView(state, cur, links)
                        cur.kind == StudyKind.FLASHCARDS -> StudyFlashcards(state, cur, links)
                        else -> StudyPlayer(state, cur, links)
                    }
                }
            }
        }
    }
}

/** Liens de l'entraînement vers les éléments de l'espace : un appui ouvre l'élément quand l'application sait l'afficher. */
@Composable
private fun LinksSection(links: List<StudyLink>) {
    if (links.isEmpty()) return
    val opener = LocalOpener.current
    Column {
        SectionTitle(stringResource(R.string.study_links))
        links.forEach { l ->
            val icon = when (l.target_kind) { "page" -> Icons.Outlined.Description; "task" -> Icons.Outlined.TaskAlt; "board" -> Icons.Outlined.ViewKanban; "event" -> Icons.Outlined.Event; else -> Icons.Outlined.Code }
            ListItem(
                headlineContent = { Text(l.title) }, leadingContent = { Icon(icon, null) },
                modifier = Modifier.clickable {
                    when (l.target_kind) { "page" -> opener.route("doc/${l.target_id}"); "task" -> opener.task(l.target_id); "board" -> opener.board(l.target_id); "project" -> opener.route("project/${l.target_id}"); "event" -> opener.route("calendar") }
                },
            )
        }
    }
}

/** QCM et questionnaires : réponses, correction (serveur, ou locale hors ligne), score. */
@Composable
private fun StudyPlayer(state: AppState, set: StudySet, links: List<StudyLink>) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val answers = remember(set.id) { mutableStateMapOf<String, JsonElement>() }
    var result by remember(set.id) { mutableStateOf<AttemptResult?>(null) }
    var started by remember(set.id) { mutableStateOf(java.time.Instant.now().toString()) }
    var busy by remember(set.id) { mutableStateOf(false) }
    var error by remember(set.id) { mutableStateOf<String?>(null) }
    val res = result
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (res != null) item(key = "score") {
            Card(Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer)) {
                Column(Modifier.padding(16.dp)) {
                    if (res.attempt.max_score > 0) Text(stringResource(R.string.study_score, res.attempt.score, res.attempt.max_score), style = MaterialTheme.typography.titleLarge)
                    else Text(stringResource(R.string.study_saved_offline))
                }
            }
        }
        itemsIndexed(set.items, key = { _, q -> q.id }) { i, q ->
            val fix = res?.correction?.firstOrNull { c -> c.item_id == q.id }
            val locked = res != null
            OutlinedCard(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Row { Text("${i + 1}.  ", style = MaterialTheme.typography.titleSmall); ChatMarkdown(q.prompt) }
                    when (q.type) {
                        "single" -> q.choices.orEmpty().forEach { c ->
                            val on = (answers[q.id] as? JsonPrimitive)?.contentOrNull == c.id
                            Row(Modifier.fillMaxWidth().clickable(enabled = !locked) { answers[q.id] = JsonPrimitive(c.id) }, verticalAlignment = Alignment.CenterVertically) {
                                RadioButton(selected = on, onClick = null, enabled = !locked); Spacer(Modifier.width(8.dp)); ChatMarkdown(c.text)
                            }
                        }
                        "multi" -> q.choices.orEmpty().forEach { c ->
                            val cur = (answers[q.id] as? JsonArray).orEmpty().mapNotNull { x -> (x as? JsonPrimitive)?.contentOrNull }
                            val on = c.id in cur
                            Row(Modifier.fillMaxWidth().clickable(enabled = !locked) { answers[q.id] = JsonArray((if (on) cur - c.id else cur + c.id).map { x -> JsonPrimitive(x) }) }, verticalAlignment = Alignment.CenterVertically) {
                                Checkbox(checked = on, onCheckedChange = null, enabled = !locked); Spacer(Modifier.width(8.dp)); ChatMarkdown(c.text)
                            }
                        }
                        else -> OutlinedTextField(
                            value = (answers[q.id] as? JsonPrimitive)?.contentOrNull.orEmpty(), onValueChange = { v -> answers[q.id] = JsonPrimitive(v) },
                            enabled = !locked, label = { Text(stringResource(R.string.study_your_answer)) }, modifier = Modifier.fillMaxWidth(), singleLine = true,
                        )
                    }
                    if (fix != null) {
                        val expected = StudyLogic.expectedText(q, fix.answer)
                        when (fix.correct) {
                            true -> Text("✓ " + stringResource(R.string.study_right), color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge)
                            false -> Text("✗ " + stringResource(R.string.study_wrong), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.labelLarge)
                            null -> Text(stringResource(R.string.study_unscored), color = MaterialTheme.colorScheme.outline, style = MaterialTheme.typography.labelLarge)
                        }
                        if (fix.correct != true && expected.isNotBlank()) { Text(stringResource(R.string.study_expected) + " :", style = MaterialTheme.typography.labelMedium); ChatMarkdown(expected) }
                        if (fix.explanation.isNotBlank()) ChatMarkdown(fix.explanation)
                    }
                }
            }
        }
        item(key = "action") {
            error?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(bottom = 8.dp)) }
            if (res == null) Button(enabled = !busy, onClick = {
                busy = true; error = null
                scope.launch {
                    state.submitStudyAttempt(set, answers.toMap(), started).onSuccess { result = it }.onFailure { e -> if (e is kotlinx.coroutines.CancellationException) throw e; error = errorText(ctx, e) }
                    busy = false
                }
            }) { Text(stringResource(R.string.study_submit)) }
            else OutlinedButton(onClick = { answers.clear(); result = null; started = java.time.Instant.now().toString() }) { Text(stringResource(R.string.study_restart)) }
        }
        item(key = "links") { LinksSection(links) }
    }
}

/** Historique des tentatives et statistiques (privé à la personne). */
@Composable
private fun StudyHistory(state: AppState, set: StudySet, links: List<StudyLink>) {
    val ctx = LocalContext.current
    var attempts by remember(set.id) { mutableStateOf<List<StudyAttempt>?>(null) }
    LaunchedEffect(set.id) { attempts = state.studyAttempts(set.id) }
    val list = attempts
    if (list == null) { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }; return }
    val st = StudyLogic.stats(list)
    LazyColumn(Modifier.fillMaxSize()) {
        item {
            Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.SpaceEvenly) {
                Stat(st.count.toString(), stringResource(R.string.study_attempts))
                Stat("${st.average}%", stringResource(R.string.study_average))
                Stat("${st.best}%", stringResource(R.string.study_best))
            }
        }
        if (list.isEmpty()) item { Text(stringResource(R.string.study_no_attempts), Modifier.padding(horizontal = 16.dp)) }
        items(list, key = { it.id }) { a ->
            ListItem(
                headlineContent = { Text(relativeTime(a.started_at, ctx)) },
                supportingContent = if (a.pending) { { Text(stringResource(R.string.study_pending)) } } else null,
                trailingContent = { if (a.max_score > 0) Text("${a.score}/${a.max_score}", style = MaterialTheme.typography.titleMedium) else if (a.pending) Icon(Icons.Outlined.Sync, null) },
            )
        }
        item { LinksSection(links) }
    }
}

@Composable
private fun Stat(value: String, label: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(value, style = MaterialTheme.typography.headlineSmall)
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
    }
}

/** Fiches : recto/verso puis évaluation. L'évaluation est gardée sur l'appareil et envoyée dès que le réseau est là. */
@Composable
private fun StudyFlashcards(state: AppState, set: StudySet, links: List<StudyLink>) {
    val scope = rememberCoroutineScope()
    var cards by remember(set.id) { mutableStateOf<List<CardState>?>(null) }
    var all by remember(set.id) { mutableStateOf(false) }
    var flipped by remember(set.id) { mutableStateOf(false) }
    var done by remember(set.id) { mutableStateOf(setOf<String>()) }
    LaunchedEffect(set.id) { cards = state.studyCards(set.id) }
    val states = cards
    if (states == null) { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }; return }
    val byItem = states.associateBy { it.item_id }
    val due = set.items.filter { it.id !in done && (all || StudyLogic.isDue(byItem[it.id])) }
    val current = due.firstOrNull()
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        item { Text(stringResource(R.string.study_due_cards, due.size), color = MaterialTheme.colorScheme.outline) }
        if (current != null) {
            item(key = "card" + current.id + flipped) {
                Card(Modifier.fillMaxWidth().heightIn(min = 180.dp).clickable { flipped = !flipped }) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(stringResource(if (flipped) R.string.study_back else R.string.study_front), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                        ChatMarkdown(if (flipped) StudyLogic.back(current) else current.prompt)
                    }
                }
            }
            item(key = "rate") {
                if (!flipped) Button(onClick = { flipped = true }, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.study_flip)) }
                else Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    listOf("again" to R.string.study_again, "hard" to R.string.study_hard, "good" to R.string.study_good, "easy" to R.string.study_easy).forEach { (rating, label) ->
                        val m = Modifier.weight(1f)
                        val rate = {
                            flipped = false; done = done + current.id
                            scope.launch { cards = state.reviewCard(set.id, current.id, rating, states) }
                            Unit
                        }
                        if (rating == "good") Button(onClick = rate, modifier = m, contentPadding = PaddingValues(horizontal = 4.dp)) { Text(stringResource(label), maxLines = 1) }
                        else OutlinedButton(onClick = rate, modifier = m, contentPadding = PaddingValues(horizontal = 4.dp)) { Text(stringResource(label), maxLines = 1) }
                    }
                }
            }
        } else item(key = "none") {
            Column(Modifier.fillMaxWidth().padding(vertical = 24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.study_none_due))
                OutlinedButton(onClick = { all = true; done = emptySet() }) { Text(stringResource(R.string.study_review_all)) }
            }
        }
        item(key = "links") { LinksSection(links) }
    }
}

/** Synthèse : sommaire, sections en Markdown, repère « lu » gardé sur l'appareil. */
@Composable
private fun StudySummaryView(state: AppState, set: StudySet, links: List<StudyLink>) {
    var read by remember(set.id) { mutableStateOf(state.studyRead(set.id)) }
    val list = rememberLazyListState()
    val scope = rememberCoroutineScope()
    LazyColumn(Modifier.fillMaxSize(), state = list, contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        item(key = "toc") {
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(12.dp)) {
                    Text(stringResource(R.string.study_toc), style = MaterialTheme.typography.titleSmall)
                    set.items.forEachIndexed { i, s ->
                        Row(Modifier.fillMaxWidth().clickable { scope.launch { list.animateScrollToItem(i + 1) } }.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                            Icon(if (s.id in read) Icons.Outlined.CheckCircle else Icons.Outlined.Circle, null, Modifier.size(16.dp)); Spacer(Modifier.width(8.dp))
                            ChatMarkdown(s.prompt)
                        }
                    }
                    Text(stringResource(R.string.study_progress, set.items.count { it.id in read }, set.items.size), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline)
                }
            }
        }
        items(set.items, key = { it.id }) { s ->
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                ChatMarkdown("## " + s.prompt.replace("\n", " "))
                ChatMarkdown(StudyLogic.back(s))
                TextButton(onClick = { read = state.toggleStudyRead(set.id, s.id) }) {
                    Icon(if (s.id in read) Icons.Outlined.CheckCircle else Icons.Outlined.Circle, null, Modifier.size(18.dp)); Spacer(Modifier.width(6.dp))
                    Text(stringResource(if (s.id in read) R.string.study_read else R.string.study_mark_read))
                }
                HorizontalDivider()
            }
        }
        item(key = "links") { LinksSection(links) }
    }
}
