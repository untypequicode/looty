package io.github.untypequicode.looty.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.NotificationsNone
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import kotlinx.serialization.json.jsonPrimitive

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InboxScreen(state: AppState, padding: PaddingValues) {
    val ctx = LocalContext.current
    val ws = state.ws ?: return
    val list = state.inbox.data.orEmpty()
    PullToRefreshBox(isRefreshing = state.inbox.loading && state.inbox.data != null, onRefresh = { state.loadInbox() }, modifier = Modifier.padding(padding)) {
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
            if (list.isEmpty() && !state.inbox.loading) item { EmptyState(Icons.Outlined.NotificationsNone, stringResource(R.string.inbox_empty)) }
            items(list, key = { it.id }) { n ->
                val unread = n.read_at == null
                val title = n.data["title"]?.jsonPrimitive?.content ?: ""
                // E-mail : l'expéditeur à la place de « Nouveau message ».
                val from = n.data["from"]?.jsonPrimitive?.content
                ListItem(
                    headlineContent = { Text(title.ifBlank { notificationText(n) }, maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = if (unread) FontWeight.SemiBold else FontWeight.Normal) },
                    supportingContent = { Text(if (n.kind == "mail" && from != null) from else notificationText(n), maxLines = 1) },
                    leadingContent = { Avatar(n.actor_name ?: from ?: "Looty", 36.dp) },
                    trailingContent = {
                        Column(horizontalAlignment = androidx.compose.ui.Alignment.End) {
                            Text(relativeTime(n.created_at, ctx), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            if (unread) Box(Modifier.padding(top = 6.dp).size(8.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
                        }
                    },
                    colors = if (unread) ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow) else ListItemDefaults.colors(),
                    modifier = Modifier.clickable { state.markRead(n); openWeb(ctx, Paths.notification(ws, n)) },
                )
            }
        }
    }
}
