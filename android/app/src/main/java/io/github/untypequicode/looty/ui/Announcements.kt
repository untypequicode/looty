package io.github.untypequicode.looty.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Announcement
import io.github.untypequicode.looty.api.Audience
import io.github.untypequicode.looty.api.Poll
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.putJsonObject
import kotlinx.serialization.json.add

/** « Pour : @TD2, Alice » ; vide : tout l'espace. */
@Composable
fun audienceLabel(state: AppState, a: Audience?): String {
    if (a == null || (a.groups.isEmpty() && a.users.isEmpty())) return stringResource(R.string.audience_everyone)
    return (a.groups.map { g -> "@" + (state.groups.firstOrNull { it.id == g }?.name ?: "…") } + a.users.map { state.members[it]?.name ?: "…" }).joinToString(", ")
}

/**
 * Annonces de l'espace (GLOSSARY) : épinglées puis récentes, lues à l'ouverture, avec leur
 * sondage. Qui a le droit « Publier des annonces » publie, modifie, épingle et supprime.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AnnouncementsScreen(state: AppState, onBack: () -> Unit) {
    val data = state.announcements.data
    val manage = data?.can_manage == true
    var editing by remember { mutableStateOf<Announcement?>(null) }
    var creating by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { state.loadAnnouncements(); state.loadMembers() }
    // Ouvrir l'écran vaut lecture des annonces qui me sont adressées.
    LaunchedEffect(data) {
        kotlinx.coroutines.delay(1500)
        data?.announcements?.filter { !it.read && !it.expired && it.for_me }?.forEach { state.markAnnouncementRead(it.id) }
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.announcements)) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
        floatingActionButton = {
            if (manage) ExtendedFloatingActionButton(onClick = { creating = true }, icon = { Icon(Icons.Outlined.Campaign, null) }, text = { Text(stringResource(R.string.announcement_new)) })
        },
    ) { padding ->
        PullToRefreshBox(isRefreshing = state.announcements.loading && data != null, onRefresh = { state.loadAnnouncements() }, modifier = Modifier.padding(padding).fillMaxSize()) {
            if (data == null) { Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }; return@PullToRefreshBox }
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(16.dp, 8.dp, 16.dp, 96.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (data.announcements.isEmpty()) item { EmptyState(Icons.Outlined.Campaign, stringResource(R.string.announcements_empty)) }
                items(data.announcements, key = { it.id }) { a -> AnnouncementCard(state, a, manage, onEdit = { editing = a }) }
            }
        }
    }
    if (creating || editing != null) AnnouncementEditor(state, editing, onDismiss = { creating = false; editing = null })
}

@Composable
fun AnnouncementCard(state: AppState, a: Announcement, manage: Boolean, onEdit: () -> Unit = {}, compact: Boolean = false) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var menu by remember { mutableStateOf(false) }
    val unread = !a.read && !a.expired && a.for_me
    ElevatedCard(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Avatar(a.author_name, 30.dp)
                Spacer(Modifier.width(10.dp))
                Column(Modifier.weight(1f)) {
                    Text(a.title, style = MaterialTheme.typography.titleMedium, fontWeight = if (unread) FontWeight.Bold else null)
                    Text(listOfNotNull(a.author_name, relativeTime(a.created_at, ctx)).joinToString(" · "), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                if (a.pinned && !a.expired) Icon(Icons.Outlined.PushPin, stringResource(R.string.announcement_pinned), Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
                if (unread) Box(Modifier.padding(start = 6.dp).size(8.dp).clip(MaterialTheme.shapes.extraSmall).background(MaterialTheme.colorScheme.primary))
                if (manage && !compact) Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                    DropdownMenu(menu, { menu = false }) {
                        DropdownMenuItem(text = { Text(stringResource(R.string.edit)) }, leadingIcon = { Icon(Icons.Outlined.Edit, null) }, onClick = { menu = false; onEdit() })
                        if (a.poll != null && !a.poll.closed) DropdownMenuItem(text = { Text(stringResource(R.string.poll_close)) }, leadingIcon = { Icon(Icons.Outlined.Lock, null) },
                            onClick = { menu = false; scope.launch { state.closePoll(a.id) } })
                        DropdownMenuItem(text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) }, leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) },
                            onClick = { menu = false; scope.launch { state.deleteAnnouncement(a.id) } })
                    }
                }
            }
            if (a.audience != null) Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Outlined.Groups, null, Modifier.size(14.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.width(4.dp))
                Text(stringResource(R.string.audience_for, audienceLabel(state, a.audience)) + if (!a.for_me) " · " + stringResource(R.string.audience_not_for_you) else "",
                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (a.expired) Text(stringResource(R.string.announcement_expired), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 4.dp))
            if (a.body.replace("<p></p>", "").isNotBlank()) Box(Modifier.padding(top = 8.dp)) {
                if (compact) Text(io.github.untypequicode.looty.ui.Html.toPlain(a.body), maxLines = 3, style = MaterialTheme.typography.bodyMedium) else HtmlBlocks(state, a.body)
            }
            if (a.poll != null && !compact) PollView(state, a.id, a.poll, a.for_me || manage)
        }
    }
}

/** Sondage d'une annonce : voter, changer ou retirer son vote, résultats selon ses règles. */
@Composable
fun PollView(state: AppState, announcementId: String, poll: Poll, canVote: Boolean) {
    val scope = rememberCoroutineScope()
    var chosen by remember(poll.my_votes) { mutableStateOf(poll.my_votes.toSet()) }
    val open = !poll.closed && canVote
    val voted = poll.my_votes.isNotEmpty()
    val total = poll.options.sumOf { it.votes ?: 0 }.coerceAtLeast(1)
    Column(Modifier.padding(top = 12.dp).fillMaxWidth().clip(MaterialTheme.shapes.medium).background(MaterialTheme.colorScheme.surfaceContainerHigh).padding(12.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Outlined.HowToVote, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.width(6.dp))
            Text(poll.question, style = MaterialTheme.typography.titleSmall, modifier = Modifier.weight(1f))
        }
        Text(listOfNotNull(
            stringResource(if (poll.multiple) R.string.poll_multiple else R.string.poll_single),
            if (poll.anonymous) stringResource(R.string.poll_anonymous) else null,
            if (poll.closed) stringResource(R.string.poll_closed) else null,
        ).joinToString(" · "), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(6.dp))
        poll.options.forEach { o ->
            val selected = o.id in chosen
            Row(Modifier.fillMaxWidth().clip(MaterialTheme.shapes.small).clickable(enabled = open) {
                chosen = if (poll.multiple) (if (selected) chosen - o.id else chosen + o.id) else setOf(o.id)
            }.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                if (poll.multiple) Checkbox(selected, null, enabled = open) else RadioButton(selected, null, enabled = open)
                Spacer(Modifier.width(8.dp))
                Column(Modifier.weight(1f)) {
                    Text(o.label)
                    if (poll.results_visible && o.votes != null) {
                        LinearProgressIndicator(progress = { o.votes.toFloat() / total }, modifier = Modifier.fillMaxWidth().padding(top = 4.dp))
                        Text("${o.votes} · ${o.votes * 100 / total} %" + (o.voters?.takeIf { it.isNotEmpty() }?.let { v -> " · " + v.joinToString(", ") { state.members[it]?.name ?: "?" } } ?: ""),
                            style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
        Text(pluralStringResource(R.plurals.poll_voters, poll.voter_count, poll.voter_count) + if (!poll.results_visible) " · " + stringResource(R.string.poll_results_after_vote) else "",
            style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(top = 6.dp))
        if (open) Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(enabled = chosen.isNotEmpty() && chosen != poll.my_votes.toSet(), onClick = { scope.launch { state.voteAnnouncement(announcementId, chosen.toList()) } }) {
                Text(stringResource(if (voted) R.string.poll_change else R.string.poll_vote))
            }
            if (voted) TextButton(onClick = { scope.launch { state.voteAnnouncement(announcementId, emptyList()) } }) { Text(stringResource(R.string.poll_retract)) }
        }
    }
}

/** Publier ou modifier une annonce : titre, texte, épinglage, destinataires, sondage. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun AnnouncementEditor(state: AppState, editing: Announcement?, onDismiss: () -> Unit) {
    val scope = rememberCoroutineScope()
    var title by remember { mutableStateOf(editing?.title.orEmpty()) }
    var body by remember { mutableStateOf(Html.toPlain(editing?.body)) }
    var pinned by remember { mutableStateOf(editing?.pinned ?: false) }
    var groups by remember { mutableStateOf(editing?.audience?.groups.orEmpty().toSet()) }
    var users by remember { mutableStateOf(editing?.audience?.users.orEmpty().toSet()) }
    var withPoll by remember { mutableStateOf(editing?.poll != null) }
    val locked = (editing?.poll?.voter_count ?: 0) > 0
    var question by remember { mutableStateOf(editing?.poll?.question.orEmpty()) }
    val options = remember { mutableStateListOf(*(editing?.poll?.options?.map { it.label } ?: listOf("", "")).toTypedArray()) }
    var multiple by remember { mutableStateOf(editing?.poll?.multiple ?: false) }
    var anonymous by remember { mutableStateOf(editing?.poll?.anonymous ?: false) }
    var saving by remember { mutableStateOf(false) }
    val valid = title.isNotBlank() && (!withPoll || (question.isNotBlank() && options.count { it.isNotBlank() } >= 2))
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 24.dp).imePadding(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(stringResource(if (editing == null) R.string.announcement_new else R.string.announcement_edit), style = MaterialTheme.typography.titleLarge)
            OutlinedTextField(title, { title = it.take(200) }, label = { Text(stringResource(R.string.title)) }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(body, { body = it }, label = { Text(stringResource(R.string.announcement_body)) }, minLines = 3, modifier = Modifier.fillMaxWidth())
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.announcement_pin), Modifier.weight(1f))
                Switch(pinned, { pinned = it })
            }
            // Destinataires : rien de choisi = tout l'espace ; les groupes sont suivis.
            Text(stringResource(R.string.audience_label), style = MaterialTheme.typography.titleSmall)
            Text(stringResource(R.string.audience_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                FilterChip(groups.isEmpty() && users.isEmpty(), onClick = { groups = emptySet(); users = emptySet() }, label = { Text(stringResource(R.string.audience_everyone)) })
                state.groups.forEach { g ->
                    FilterChip(g.id in groups, onClick = { groups = if (g.id in groups) groups - g.id else groups + g.id }, label = { Text("@" + g.name) })
                }
                state.members.values.sortedBy { it.name }.forEach { m ->
                    FilterChip(m.user_id in users, onClick = { users = if (m.user_id in users) users - m.user_id else users + m.user_id }, label = { Text(m.name) })
                }
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.poll_add), Modifier.weight(1f))
                Switch(withPoll, { withPoll = it }, enabled = !locked)
            }
            if (withPoll) {
                OutlinedTextField(question, { question = it.take(300) }, label = { Text(stringResource(R.string.poll_question)) }, modifier = Modifier.fillMaxWidth())
                if (locked) Text(stringResource(R.string.poll_locked), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                options.forEachIndexed { i, o ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        OutlinedTextField(o, { options[i] = it.take(200) }, enabled = !locked, singleLine = true, placeholder = { Text(stringResource(R.string.poll_option, i + 1)) }, modifier = Modifier.weight(1f))
                        if (options.size > 2 && !locked) IconButton(onClick = { options.removeAt(i) }) { Icon(Icons.Outlined.Close, stringResource(R.string.clear)) }
                    }
                }
                if (options.size < 10 && !locked) TextButton(onClick = { options.add("") }) { Icon(Icons.Outlined.Add, null); Spacer(Modifier.width(4.dp)); Text(stringResource(R.string.poll_add_option)) }
                Row(verticalAlignment = Alignment.CenterVertically) { Text(stringResource(R.string.poll_multiple), Modifier.weight(1f)); Switch(multiple, { multiple = it }, enabled = !locked) }
                Row(verticalAlignment = Alignment.CenterVertically) { Text(stringResource(R.string.poll_anonymous), Modifier.weight(1f)); Switch(anonymous, { anonymous = it }, enabled = !locked) }
            }
            Button(enabled = valid && !saving, modifier = Modifier.align(Alignment.End), onClick = {
                saving = true
                val req = buildJsonObject {
                    put("title", title.trim())
                    put("body", Html.fromPlain(body))
                    put("pinned", pinned)
                    putJsonObject("audience") {
                        putJsonArray("groups") { groups.forEach { add(it) } }
                        putJsonArray("users") { users.forEach { add(it) } }
                    }
                    when {
                        withPoll -> putJsonObject("poll") {
                            put("question", question.trim())
                            putJsonArray("options") { options.filter { it.isNotBlank() }.forEach { add(it.trim()) } }
                            put("multiple", multiple); put("anonymous", anonymous)
                            put("results_after_vote", editing?.poll?.results_after_vote ?: false)
                            put("closes_at", editing?.poll?.closes_at)
                        }
                        editing?.poll != null -> put("poll", JsonNull)
                    }
                }
                scope.launch { if (state.saveAnnouncement(editing?.id, req)) onDismiss(); saving = false }
            }) { Text(stringResource(if (editing == null) R.string.announcement_publish else R.string.save)) }
        }
    }
}
