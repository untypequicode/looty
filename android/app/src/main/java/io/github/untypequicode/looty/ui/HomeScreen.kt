package io.github.untypequicode.looty.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import java.time.Instant
import java.time.LocalTime
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(state: AppState, padding: PaddingValues, onTasks: () -> Unit) {
    val ctx = LocalContext.current
    val ws = state.ws ?: return
    val first = state.me?.name?.split(" ")?.firstOrNull().orEmpty()
    val hour = LocalTime.now().hour
    val greeting = stringResource(if (hour < 12) R.string.greeting_morning else if (hour < 18) R.string.greeting_afternoon else R.string.greeting_evening, first)
    val counts = state.overview.data?.my
    val opener = LocalOpener.current
    PullToRefreshBox(isRefreshing = state.overview.loading && state.overview.data != null, onRefresh = { state.refreshAll() }, modifier = Modifier.padding(padding)) {
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
            item {
                Text(greeting, style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 24.dp, bottom = 12.dp))
            }
            item {
                Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Stat(Icons.Outlined.AccountCircle, counts?.open, stringResource(R.string.stat_assigned), MaterialTheme.colorScheme.primary, Modifier.weight(1f), onTasks)
                        Stat(Icons.Outlined.WarningAmber, counts?.overdue, stringResource(R.string.stat_overdue), MaterialTheme.colorScheme.error, Modifier.weight(1f), onTasks)
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Stat(Icons.Outlined.Event, counts?.due_week, stringResource(R.string.stat_week), MaterialTheme.colorScheme.tertiary, Modifier.weight(1f), onTasks)
                        Stat(Icons.Outlined.TaskAlt, counts?.completed_week, stringResource(R.string.stat_done), MaterialTheme.colorScheme.secondary, Modifier.weight(1f), onTasks)
                    }
                }
            }
            item { SectionTitle(stringResource(R.string.upcoming), action = { TextButton(onClick = { opener.route("calendar") }) { Text(stringResource(R.string.view_all)) } }) }
            val events = state.upcoming.data.orEmpty().take(5)
            if (events.isEmpty()) item { Text(stringResource(R.string.no_events), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp)) }
            items(events, key = { it.id }) { e ->
                val start = Instant.parse(e.starts_at).atZone(ZoneId.systemDefault())
                ListItem(
                    headlineContent = { Text(e.summary, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = e.location?.let { { Text(it, maxLines = 1) } },
                    leadingContent = { Icon(Icons.Outlined.CalendarMonth, null, tint = MaterialTheme.colorScheme.primary) },
                    trailingContent = {
                        Column(horizontalAlignment = Alignment.End) {
                            Text(if (e.all_day) stringResource(R.string.all_day) else start.format(DateTimeFormatter.ofPattern("HH:mm")), style = MaterialTheme.typography.labelLarge)
                            Text(dayLabel(start.toLocalDate()), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    },
                    modifier = Modifier.clickable { opener.route("calendar") },
                )
            }
            item { SectionTitle(stringResource(R.string.my_tasks), action = { TextButton(onClick = onTasks) { Text(stringResource(R.string.view_all)) } }) }
            val tasks = state.tasks.data.orEmpty().filter { it.assigned_to_me && !it.done_by_me }.take(6)
            if (tasks.isEmpty() && !state.tasks.loading) item { Text(stringResource(R.string.no_tasks), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp)) }
            items(tasks, key = { "t" + it.id }) { t -> TaskRow(state, t, compact = true) }
            val boards = state.overview.data?.boards.orEmpty()
            if (boards.isNotEmpty()) {
                item { SectionTitle(stringResource(R.string.boards)) }
                item {
                    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        items(boards, key = { it.id }) { b ->
                            ElevatedCard(onClick = { opener.board(b.id) }, modifier = Modifier.width(200.dp)) {
                                Column(Modifier.padding(14.dp)) {
                                    Text(b.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                    Spacer(Modifier.height(10.dp))
                                    LinearProgressIndicator(progress = { if (b.total == 0) 0f else b.done.toFloat() / b.total }, modifier = Modifier.fillMaxWidth())
                                    Spacer(Modifier.height(8.dp))
                                    Text("${b.total - b.done} · ${b.overdue}", style = MaterialTheme.typography.labelMedium, color = if (b.overdue > 0) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Stat(icon: ImageVector, value: Int?, label: String, tint: Color, modifier: Modifier, onClick: () -> Unit) {
    Card(onClick = onClick, modifier = modifier, colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)) {
        Column(Modifier.padding(14.dp)) {
            Icon(icon, null, tint = tint)
            Spacer(Modifier.height(8.dp))
            Text(value?.toString() ?: "–", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold)
            Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}
