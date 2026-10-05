package io.github.untypequicode.looty.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

private val FREQS = listOf("none" to R.string.repeat_none, "daily" to R.string.repeat_daily, "weekly" to R.string.repeat_weekly, "monthly" to R.string.repeat_monthly, "yearly" to R.string.repeat_yearly)

/**
 * Création / modification d'un événement du calendrier de l'espace (mêmes règles que le
 * formulaire web : fin incluse pour une journée entière, règle complexe conservée telle quelle).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EventEditor(state: AppState, eventId: String?, date: LocalDate, onDone: () -> Unit) {
    val zone = ZoneId.systemDefault()
    val scope = rememberCoroutineScope()
    var loaded by rememberSaveable { mutableStateOf(eventId == null) }
    var title by rememberSaveable { mutableStateOf("") }
    var allDay by rememberSaveable { mutableStateOf(false) }
    val defaultStart = if (date == LocalDate.now()) LocalTime.now().plusHours(1).withMinute(0) else LocalTime.of(9, 0)
    var start by rememberSaveable { mutableStateOf(date.atTime(defaultStart).toString()) }
    var end by rememberSaveable { mutableStateOf(date.atTime(defaultStart).plusHours(1).toString()) }
    var freq by rememberSaveable { mutableStateOf("none") }
    var interval by rememberSaveable { mutableStateOf("1") }
    var until by rememberSaveable { mutableStateOf<String?>(null) }
    var complex by rememberSaveable { mutableStateOf(false) }
    var location by rememberSaveable { mutableStateOf("") }
    var description by rememberSaveable { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var picker by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(eventId) {
        if (eventId == null || loaded) return@LaunchedEffect
        val e = state.eventDetail(eventId)
        if (e == null) { error = "not_found"; return@LaunchedEffect }
        title = e.title
        allDay = e.all_day
        val s = Instant.parse(e.start)
        val en = Instant.parse(e.end)
        if (e.all_day) {
            start = s.atZone(ZoneOffset.UTC).toLocalDate().atTime(9, 0).toString()
            end = en.atZone(ZoneOffset.UTC).toLocalDate().minusDays(1).atTime(10, 0).toString()
        } else {
            start = s.atZone(zone).toLocalDateTime().toString()
            end = en.atZone(zone).toLocalDateTime().toString()
        }
        complex = e.complex_recurrence
        freq = when { e.repeat != null -> e.repeat.freq; complex -> "custom"; else -> "none" }
        interval = (e.repeat?.interval ?: 1).toString()
        until = e.repeat?.until
        location = e.location.orEmpty()
        description = e.description.orEmpty()
        loaded = true
    }

    val s = LocalDateTime.parse(start)
    val en = LocalDateTime.parse(end)
    val valid = title.isNotBlank() && if (allDay) en.toLocalDate() >= s.toLocalDate() else en >= s
    val dateFmt = DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM)
    val timeFmt = DateTimeFormatter.ofPattern("HH:mm")

    fun save() {
        busy = true; error = null
        val body = buildJsonObject {
            put("title", title.trim())
            put("all_day", allDay)
            put("start", if (allDay) s.toLocalDate().toString() else s.atZone(zone).toInstant().toString())
            put("end", if (allDay) en.toLocalDate().toString() else en.atZone(zone).toInstant().toString())
            put("tz", zone.id)
            if (location.isNotBlank()) put("location", location.trim())
            if (description.isNotBlank()) put("description", description.trim())
            if (freq != "none" && freq != "custom") putJsonObject("repeat") {
                put("freq", freq)
                put("interval", interval.toIntOrNull()?.coerceIn(1, 365) ?: 1)
                until?.let { put("until", it) }
            }
            put("keep_recurrence", freq == "custom")
        }
        scope.launch {
            state.saveEvent(eventId, body).onSuccess { onDone() }.onFailure { error = (it as? io.github.untypequicode.looty.api.ApiException)?.code ?: "offline" }
            busy = false
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(if (eventId == null) R.string.new_event else R.string.edit_event)) },
                navigationIcon = { IconButton(onClick = onDone) { Icon(Icons.Filled.Close, stringResource(R.string.cancel)) } },
                actions = {
                    Button(onClick = ::save, enabled = valid && loaded && !busy, modifier = Modifier.padding(end = 8.dp)) { Text(stringResource(R.string.save)) }
                },
            )
        },
    ) { padding ->
        if (!loaded && error == null) {
            Box(Modifier.padding(padding).fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            return@Scaffold
        }
        Column(Modifier.padding(padding).fillMaxSize().imePadding().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            error?.let { Text(stringResource(if (it == "event_invalid") R.string.event_invalid else R.string.error_generic) + " ($it)", color = MaterialTheme.colorScheme.error) }
            OutlinedTextField(title, { title = it }, Modifier.fillMaxWidth(), label = { Text(stringResource(R.string.event_title)) }, singleLine = true,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences), textStyle = MaterialTheme.typography.titleLarge)
            Text(stringResource(if (eventId != null && freq != "none") R.string.series_hint else R.string.shared_event_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)

            Row(Modifier.fillMaxWidth().clickable { allDay = !allDay }.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Outlined.Schedule, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.width(16.dp))
                Text(stringResource(R.string.all_day), Modifier.weight(1f))
                Switch(allDay, { allDay = it })
            }
            @Composable
            fun whenRow(label: Int, v: LocalDateTime, key: String) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(label), Modifier.width(72.dp).padding(start = 40.dp), color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium)
                    Spacer(Modifier.weight(1f))
                    TextButton(onClick = { picker = "${key}Date" }) { Text(v.toLocalDate().format(dateFmt)) }
                    if (!allDay) TextButton(onClick = { picker = "${key}Time" }) { Text(v.format(timeFmt)) }
                }
            }
            whenRow(R.string.starts, s, "start")
            whenRow(R.string.ends, en, "end")

            HorizontalDivider()
            var freqMenu by remember { mutableStateOf(false) }
            Row(Modifier.fillMaxWidth().clickable { freqMenu = true }.padding(vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Outlined.Repeat, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.width(16.dp))
                Box(Modifier.weight(1f)) {
                    Text(stringResource(FREQS.firstOrNull { it.first == freq }?.second ?: R.string.repeat_custom))
                    DropdownMenu(freqMenu, { freqMenu = false }) {
                        (FREQS + if (complex) listOf("custom" to R.string.repeat_custom) else emptyList()).forEach { (f, label) ->
                            DropdownMenuItem(text = { Text(stringResource(label)) }, onClick = { freq = f; freqMenu = false })
                        }
                    }
                }
            }
            if (freq != "none" && freq != "custom") {
                Row(Modifier.fillMaxWidth().padding(start = 40.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    OutlinedTextField(interval, { v -> interval = v.filter { it.isDigit() }.take(3) }, Modifier.width(110.dp), label = { Text(stringResource(R.string.repeat_every)) },
                        singleLine = true, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number))
                    AssistChip(onClick = { picker = "until" }, label = { Text(until?.let { stringResource(R.string.repeat_until, LocalDate.parse(it).format(dateFmt)) } ?: stringResource(R.string.repeat_forever)) },
                        trailingIcon = if (until != null) ({ Icon(Icons.Filled.Close, stringResource(R.string.clear), Modifier.size(18.dp).clickable { until = null }) }) else null)
                }
            }
            if (freq == "custom") Text(stringResource(R.string.repeat_custom_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(start = 40.dp))

            HorizontalDivider()
            OutlinedTextField(location, { location = it }, Modifier.fillMaxWidth(), label = { Text(stringResource(R.string.location)) }, singleLine = true,
                leadingIcon = { Icon(Icons.Outlined.Place, null) })
            OutlinedTextField(description, { description = it }, Modifier.fillMaxWidth().heightIn(min = 120.dp), label = { Text(stringResource(R.string.description)) },
                leadingIcon = { Icon(Icons.Outlined.Notes, null) }, keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences))
            Spacer(Modifier.height(24.dp))
        }
    }

    when (val p = picker) {
        "startDate", "endDate", "until" -> {
            val initial = when (p) { "startDate" -> s.toLocalDate(); "endDate" -> en.toLocalDate(); else -> until?.let(LocalDate::parse) ?: s.toLocalDate().plusMonths(1) }
            val dp = rememberDatePickerState(initialSelectedDateMillis = initial.atStartOfDay().toInstant(ZoneOffset.UTC).toEpochMilli())
            DatePickerDialog(
                onDismissRequest = { picker = null },
                confirmButton = {
                    TextButton(onClick = {
                        dp.selectedDateMillis?.let { ms ->
                            val d = Instant.ofEpochMilli(ms).atZone(ZoneOffset.UTC).toLocalDate()
                            when (p) {
                                // Déplacer le début garde la durée de l'événement.
                                "startDate" -> { val dur = java.time.Duration.between(s, en); val ns = d.atTime(s.toLocalTime()); start = ns.toString(); end = ns.plus(dur).toString() }
                                "endDate" -> end = d.atTime(en.toLocalTime()).toString()
                                else -> until = d.toString()
                            }
                        }
                        picker = null
                    }) { Text("OK") }
                },
                dismissButton = { TextButton(onClick = { picker = null }) { Text(stringResource(R.string.cancel)) } },
            ) { DatePicker(dp) }
        }
        "startTime", "endTime" -> {
            val v = if (p == "startTime") s else en
            val tp = rememberTimePickerState(v.hour, v.minute, is24Hour = true)
            AlertDialog(
                onDismissRequest = { picker = null },
                confirmButton = {
                    TextButton(onClick = {
                        val t = LocalTime.of(tp.hour, tp.minute)
                        if (p == "startTime") { val dur = java.time.Duration.between(s, en); val ns = s.toLocalDate().atTime(t); start = ns.toString(); end = ns.plus(dur).toString() }
                        else end = en.toLocalDate().atTime(t).toString()
                        picker = null
                    }) { Text("OK") }
                },
                dismissButton = { TextButton(onClick = { picker = null }) { Text(stringResource(R.string.cancel)) } },
                text = { TimePicker(tp) },
            )
        }
    }
}
