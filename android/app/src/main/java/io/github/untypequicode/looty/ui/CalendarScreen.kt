package io.github.untypequicode.looty.ui

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.CalEvent
import io.github.untypequicode.looty.api.CalTask
import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.YearMonth
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.time.format.TextStyle
import java.time.temporal.TemporalAdjusters
import java.util.Locale

/** Calendrier de l'espace (événements Looty modifiables) : identifiant côté API. */
const val WORKSPACE_CAL = "workspace"

/** Jours couverts par un événement : [début, fin[ ; « journée entière » = dates à minuit UTC. */
fun CalEvent.days(): Pair<LocalDate, LocalDate> {
    val s = Instant.parse(starts_at)
    val e = Instant.parse(ends_at)
    if (all_day) return s.atZone(ZoneOffset.UTC).toLocalDate() to e.atZone(ZoneOffset.UTC).toLocalDate()
    val zone = ZoneId.systemDefault()
    return s.atZone(zone).toLocalDate() to e.atZone(zone).toLocalDate()
}

fun CalEvent.onDay(day: LocalDate): Boolean {
    val (s, e) = days()
    if (all_day) return s <= day && e > day
    val zone = ZoneId.systemDefault()
    val d0 = day.atStartOfDay(zone).toInstant()
    val d1 = day.plusDays(1).atStartOfDay(zone).toInstant()
    val st = Instant.parse(starts_at)
    val en = Instant.parse(ends_at)
    return st < d1 && (en > d0 || (en == st && st >= d0))
}

private val timeFmt = DateTimeFormatter.ofPattern("HH:mm")

@Composable
fun eventTime(e: CalEvent): String {
    if (e.all_day) return stringResource(R.string.all_day)
    val zone = ZoneId.systemDefault()
    return "${Instant.parse(e.starts_at).atZone(zone).format(timeFmt)} – ${Instant.parse(e.ends_at).atZone(zone).format(timeFmt)}"
}

fun parseColor(hex: String, fallback: Color): Color =
    runCatching { Color(android.graphics.Color.parseColor(hex.take(7))) }.getOrDefault(fallback)

/** Agenda (jours à venir) ou mois, filtre des calendriers, détail d'un événement. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CalendarScreen(state: AppState, onBack: () -> Unit, onEdit: (id: String?, date: LocalDate) -> Unit) {
    val opener = LocalOpener.current
    var month by rememberSaveable { mutableStateOf(false) }
    var selected by rememberSaveable { mutableStateOf(LocalDate.now().toString()) }
    var filters by remember { mutableStateOf(false) }
    var open by remember { mutableStateOf<CalEvent?>(null) }
    val day = LocalDate.parse(selected)
    LaunchedEffect(Unit) { state.loadCalendars() }

    val primary = MaterialTheme.colorScheme.primary
    val colors = remember(state.calSources, primary) { state.calSources.associate { it.id to parseColor(it.color, primary) } + (WORKSPACE_CAL to primary) }
    val events = state.calEvents.data?.events.orEmpty().filter { it.calendar_id !in state.hiddenCalendars }
    val tasks = if ("tasks" in state.hiddenCalendars) emptyList() else state.calEvents.data?.tasks.orEmpty()

    val pagerStart = 1200
    val pager = rememberPagerState(initialPage = pagerStart) { 2400 }
    val shownMonth = YearMonth.now().plusMonths((pager.currentPage - pagerStart).toLong())

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(if (month) shownMonth.atDay(1).format(DateTimeFormatter.ofPattern("LLLL yyyy")).replaceFirstChar { it.titlecase() } else stringResource(R.string.calendar))
                },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    val scope = rememberCoroutineScope()
                    IconButton(onClick = {
                        selected = LocalDate.now().toString()
                        if (month) scope.launch { pager.animateScrollToPage(pagerStart) } else state.calRange?.let { if (it.first != LocalDate.now()) state.loadEvents(LocalDate.now(), LocalDate.now().plusDays(60)) }
                    }) { Icon(Icons.Outlined.Today, stringResource(R.string.today)) }
                    IconButton(onClick = { month = !month }) {
                        Icon(if (month) Icons.Outlined.ViewAgenda else Icons.Outlined.CalendarViewMonth, stringResource(if (month) R.string.agenda else R.string.month))
                    }
                    IconButton(onClick = { filters = true }) { Icon(Icons.Outlined.FilterList, stringResource(R.string.calendars)) }
                },
            )
        },
        floatingActionButton = {
            if (state.canWrite) ExtendedFloatingActionButton(onClick = { onEdit(null, if (month) day else LocalDate.now()) }, icon = { Icon(Icons.Filled.Add, null) }, text = { Text(stringResource(R.string.new_event)) })
        },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            if (state.calEvents.loading) LinearProgressIndicator(Modifier.fillMaxWidth())
            if (month) {
                LaunchedEffect(shownMonth) {
                    val first = shownMonth.atDay(1).with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
                    state.loadEvents(first, first.plusDays(42))
                    if (YearMonth.from(day) != shownMonth) selected = (if (shownMonth == YearMonth.now()) LocalDate.now() else shownMonth.atDay(1)).toString()
                }
                HorizontalPager(pager, beyondViewportPageCount = 0, verticalAlignment = Alignment.Top) { page ->
                    MonthGrid(YearMonth.now().plusMonths((page - pagerStart).toLong()), day, events, tasks, colors) { selected = it.toString() }
                }
                HorizontalDivider()
                DayList(day, events, tasks, colors, header = true, modifier = Modifier.weight(1f), onEvent = { open = it }, onTask = { opener.task(it.id) })
            } else {
                LaunchedEffect(Unit) { state.loadEvents(LocalDate.now(), LocalDate.now().plusDays(60)) }
                Agenda(state, events, tasks, colors, onEvent = { open = it }, onTask = { opener.task(it.id) })
            }
        }
    }

    if (filters) CalendarFilterSheet(state, colors) { filters = false }
    open?.let { e -> EventSheet(state, e, colors[e.calendar_id] ?: primary, onDismiss = { open = null }, onEdit = { open = null; onEdit(e.event_id, e.days().first) }) }
}

@Composable
private fun MonthGrid(ym: YearMonth, selected: LocalDate, events: List<CalEvent>, tasks: List<CalTask>, colors: Map<String, Color>, onPick: (LocalDate) -> Unit) {
    val first = ym.atDay(1).with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
    val locale = Locale.getDefault()
    Column(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp)) {
        Row {
            (0..6).forEach { i ->
                Text(DayOfWeek.MONDAY.plus(i.toLong()).getDisplayName(TextStyle.NARROW, locale), Modifier.weight(1f), textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        (0 until 6).forEach { w ->
            Row {
                (0..6).forEach { i ->
                    val d = first.plusDays((w * 7 + i).toLong())
                    val inMonth = YearMonth.from(d) == ym
                    val isSel = d == selected
                    val isToday = d == LocalDate.now()
                    val dots = events.filter { it.onDay(d) }.map { colors[it.calendar_id] ?: MaterialTheme.colorScheme.primary }.distinct().take(3)
                    val hasTask = tasks.any { localDate(it.due_date) == d }
                    Column(
                        Modifier.weight(1f).height(48.dp).clip(RoundedCornerShape(12.dp)).clickable { onPick(d) },
                        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
                    ) {
                        Box(
                            Modifier.size(30.dp).clip(CircleShape)
                                .background(if (isSel) MaterialTheme.colorScheme.primary else Color.Transparent)
                                .then(if (isToday && !isSel) Modifier.border(1.5.dp, MaterialTheme.colorScheme.primary, CircleShape) else Modifier),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(d.dayOfMonth.toString(), style = MaterialTheme.typography.bodyMedium,
                                fontWeight = if (isToday) FontWeight.Bold else FontWeight.Normal,
                                color = when { isSel -> MaterialTheme.colorScheme.onPrimary; inMonth -> MaterialTheme.colorScheme.onSurface; else -> MaterialTheme.colorScheme.outline })
                        }
                        Row(Modifier.height(8.dp).padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                            dots.forEach { c -> Box(Modifier.size(5.dp).clip(CircleShape).background(c)) }
                            if (hasTask) Box(Modifier.size(5.dp).clip(CircleShape).border(1.dp, MaterialTheme.colorScheme.outline, CircleShape))
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun DayList(day: LocalDate, events: List<CalEvent>, tasks: List<CalTask>, colors: Map<String, Color>, header: Boolean, modifier: Modifier, onEvent: (CalEvent) -> Unit, onTask: (CalTask) -> Unit) {
    val list = events.filter { it.onDay(day) }.sortedWith(compareBy<CalEvent>({ !it.all_day }, { it.starts_at }))
    val due = tasks.filter { localDate(it.due_date) == day }
    LazyColumn(modifier.fillMaxWidth(), contentPadding = PaddingValues(bottom = 96.dp)) {
        if (header) item { SectionTitle(day.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.FULL)).replaceFirstChar { it.titlecase() }) }
        if (list.isEmpty() && due.isEmpty()) item { Text(stringResource(R.string.no_events_day), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp)) }
        items(list, key = { it.id }) { e -> EventRow(e, colors[e.calendar_id] ?: MaterialTheme.colorScheme.primary) { onEvent(e) } }
        items(due, key = { "t" + it.id }) { t -> CalTaskRow(t) { onTask(t) } }
    }
}

@Composable
private fun Agenda(state: AppState, events: List<CalEvent>, tasks: List<CalTask>, colors: Map<String, Color>, onEvent: (CalEvent) -> Unit, onTask: (CalTask) -> Unit) {
    val range = state.calRange
    val today = LocalDate.now()
    val end = range?.second ?: today.plusDays(60)
    val days = generateSequence(today) { it.plusDays(1) }.takeWhile { it < end }.toList()
    val byDay = days.map { d -> Triple(d, events.filter { it.onDay(d) }.sortedWith(compareBy<CalEvent>({ !it.all_day }, { it.starts_at })), tasks.filter { localDate(it.due_date) == d }) }
        .filter { it.second.isNotEmpty() || it.third.isNotEmpty() || it.first == today }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
        byDay.forEach { (d, evs, due) ->
            item(key = "d$d") {
                SectionTitle(if (d == today || d == today.plusDays(1)) "${dayLabel(d)} · ${d.format(DateTimeFormatter.ofPattern("EEE d MMM"))}" else d.format(DateTimeFormatter.ofPattern("EEEE d MMMM")).replaceFirstChar { it.titlecase() })
            }
            if (evs.isEmpty() && due.isEmpty()) item(key = "e$d") { Text(stringResource(R.string.no_events_day), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp)) }
            items(evs, key = { "$d${it.id}" }) { e -> EventRow(e, colors[e.calendar_id] ?: MaterialTheme.colorScheme.primary) { onEvent(e) } }
            items(due, key = { "$d t${it.id}" }) { t -> CalTaskRow(t) { onTask(t) } }
        }
        if (range != null && !state.calEvents.loading) item(key = "more") {
            Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                OutlinedButton(onClick = { state.loadEvents(today, end.plusDays(60)) }) { Text(stringResource(R.string.load_more)) }
            }
        }
    }
}

@Composable
private fun EventRow(e: CalEvent, color: Color, onClick: () -> Unit) {
    ListItem(
        headlineContent = { Text(e.summary.ifBlank { stringResource(R.string.untitled) }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = { Text(listOfNotNull(eventTime(e), e.location?.takeIf { it.isNotBlank() }).joinToString(" · "), maxLines = 1, overflow = TextOverflow.Ellipsis) },
        leadingContent = { Box(Modifier.width(4.dp).height(36.dp).clip(RoundedCornerShape(2.dp)).background(color)) },
        trailingContent = if (e.recurring) ({ Icon(Icons.Outlined.Repeat, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.outline) }) else null,
        modifier = Modifier.clickable(onClick = onClick),
    )
}

@Composable
private fun CalTaskRow(t: CalTask, onClick: () -> Unit) {
    ListItem(
        headlineContent = { Text(t.title, maxLines = 1, overflow = TextOverflow.Ellipsis, textDecoration = if (t.is_completed) TextDecoration.LineThrough else null) },
        supportingContent = { Text("${stringResource(R.string.due)} · ${t.board_name}", maxLines = 1) },
        leadingContent = { Icon(if (t.is_completed) Icons.Outlined.TaskAlt else Icons.Outlined.RadioButtonUnchecked, null, tint = MaterialTheme.colorScheme.onSurfaceVariant) },
        modifier = Modifier.clickable(onClick = onClick),
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun CalendarFilterSheet(state: AppState, colors: Map<String, Color>, onDismiss: () -> Unit) {
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
            Text(stringResource(R.string.calendars), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            @Composable
            fun row(id: String, name: String, color: Color, sub: String? = null) {
                val shown = id !in state.hiddenCalendars
                ListItem(
                    headlineContent = { Text(name) },
                    supportingContent = sub?.let { { Text(it, color = MaterialTheme.colorScheme.error, maxLines = 1) } },
                    leadingContent = { Checkbox(shown, { state.toggleCalendar(id) }, colors = CheckboxDefaults.colors(checkedColor = color, uncheckedColor = color)) },
                    colors = sheetItemColors(),
                    modifier = Modifier.clickable { state.toggleCalendar(id) },
                )
            }
            row(WORKSPACE_CAL, stringResource(R.string.workspace_calendar), MaterialTheme.colorScheme.primary)
            state.calSources.filter { it.scope == "workspace" }.forEach { row(it.id, it.name, colors[it.id] ?: MaterialTheme.colorScheme.primary, it.last_error) }
            val personal = state.calSources.filter { it.scope != "workspace" }
            if (personal.isNotEmpty()) {
                SectionTitle(stringResource(R.string.personal_calendars))
                personal.forEach { row(it.id, it.name, colors[it.id] ?: MaterialTheme.colorScheme.primary, it.last_error) }
            }
            HorizontalDivider(Modifier.padding(vertical = 8.dp))
            row("tasks", stringResource(R.string.show_due_tasks), MaterialTheme.colorScheme.outline)
            val ctx = LocalContext.current
            TextButton(onClick = { onDismiss(); state.ws?.let { openWeb(ctx, Paths.screen(it, "calendar")) } }, modifier = Modifier.padding(horizontal = 12.dp)) {
                Text(stringResource(R.string.manage_calendars))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun EventSheet(state: AppState, e: CalEvent, color: Color, onDismiss: () -> Unit, onEdit: () -> Unit) {
    val ctx = LocalContext.current
    var confirm by remember { mutableStateOf(false) }
    val calName = if (e.calendar_id == WORKSPACE_CAL) stringResource(R.string.workspace_calendar) else state.calSources.firstOrNull { it.id == e.calendar_id }?.name.orEmpty()
    val (d0, d1) = e.days()
    val lastDay = if (e.all_day) d1.minusDays(1) else d1
    val fmt = DateTimeFormatter.ofLocalizedDate(FormatStyle.FULL)
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(horizontal = 24.dp).padding(bottom = 16.dp)) {
            Row(verticalAlignment = Alignment.Top) {
                Box(Modifier.padding(top = 8.dp).size(14.dp).clip(RoundedCornerShape(4.dp)).background(color))
                Spacer(Modifier.width(16.dp))
                Column(Modifier.weight(1f)) {
                    Text(e.summary.ifBlank { stringResource(R.string.untitled) }, style = MaterialTheme.typography.headlineSmall)
                    Text(
                        if (lastDay > d0) "${d0.format(fmt)} – ${lastDay.format(fmt)}" else d0.format(fmt).replaceFirstChar { it.titlecase() },
                        style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(eventTime(e), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Spacer(Modifier.height(12.dp))
            @Composable
            fun line(icon: androidx.compose.ui.graphics.vector.ImageVector, text: String, onClick: (() -> Unit)? = null) {
                Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).then(if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier).padding(vertical = 10.dp), verticalAlignment = Alignment.Top) {
                    Icon(icon, null, Modifier.size(20.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    Spacer(Modifier.width(16.dp))
                    Text(text, style = MaterialTheme.typography.bodyMedium, color = if (onClick != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface)
                }
            }
            if (calName.isNotEmpty()) line(Icons.Outlined.CalendarMonth, calName)
            if (e.recurring) line(Icons.Outlined.Repeat, stringResource(R.string.recurring_event))
            e.location?.takeIf { it.isNotBlank() }?.let { loc ->
                line(Icons.Outlined.Place, loc) { runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("geo:0,0?q=" + Uri.encode(loc)))) } }
            }
            e.url?.takeIf { it.startsWith("http") }?.let { u -> line(Icons.Outlined.Link, u) { runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(u))) } } }
            e.description?.takeIf { it.isNotBlank() }?.let { line(Icons.Outlined.Notes, it) }
            if (e.can_edit && e.event_id != null) {
                Spacer(Modifier.height(12.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = onEdit, modifier = Modifier.weight(1f)) { Icon(Icons.Outlined.Edit, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.edit)) }
                    OutlinedButton(onClick = { confirm = true }, modifier = Modifier.weight(1f), colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error)) {
                        Icon(Icons.Outlined.Delete, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.delete))
                    }
                }
            }
        }
    }
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            title = { Text(stringResource(R.string.delete_event_q)) },
            text = { Text(stringResource(if (e.recurring) R.string.delete_recurring_hint else R.string.delete_event_hint)) },
            confirmButton = {
                Column(horizontalAlignment = Alignment.End) {
                    if (e.recurring) TextButton(onClick = { confirm = false; state.deleteEvent(e, onlyThis = true); onDismiss() }) { Text(stringResource(R.string.delete_this_occurrence)) }
                    TextButton(onClick = { confirm = false; state.deleteEvent(e, onlyThis = false); onDismiss() }) {
                        Text(stringResource(if (e.recurring) R.string.delete_all_occurrences else R.string.delete), color = MaterialTheme.colorScheme.error)
                    }
                    TextButton(onClick = { confirm = false }) { Text(stringResource(R.string.cancel)) }
                }
            },
        )
    }
}
