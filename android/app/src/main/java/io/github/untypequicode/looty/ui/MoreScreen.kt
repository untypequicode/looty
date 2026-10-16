package io.github.untypequicode.looty.ui

import android.content.Intent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Logout
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.BuildConfig
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.Session
import io.github.untypequicode.looty.SetupActivity
import kotlinx.coroutines.launch

@Composable
fun MoreScreen(state: AppState, padding: PaddingValues, onSignedOut: () -> Unit) {
    val ctx = LocalContext.current
    val s = state.session as? Session.Ready ?: return
    val ws = s.workspaceId
    val opener = LocalOpener.current
    var newBoard by remember { mutableStateOf(false) }
    val boardScope = rememberCoroutineScope()
    if (newBoard) NameDialog(stringResource(R.string.new_board), stringResource(R.string.board_name), "", onDismiss = { newBoard = false }) { name ->
        newBoard = false
        boardScope.launch { state.createBoard(name)?.let { opener.board(it) } }
    }
    val links: List<Triple<ImageVector, Int, String>> = listOf(
        Triple(Icons.Outlined.Hub, R.string.graph, "graph"),
        Triple(Icons.Outlined.History, R.string.activity, "activity"),
        Triple(Icons.Outlined.AutoAwesome, R.string.whats_new, "whats-new"),
        Triple(Icons.Outlined.Delete, R.string.trash, "trash"),
        Triple(Icons.Outlined.Settings, R.string.settings, "settings"),
    )
    LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
        item {
            ListItem(
                headlineContent = { Text(s.me.name) }, supportingContent = { Text(s.me.email) },
                leadingContent = { Avatar(s.me.name, 44.dp) },
            )
        }
        if (s.workspaces.size > 1) {
            item { SectionTitle(stringResource(R.string.workspace)) }
            items(s.workspaces, key = { it.id }) { w ->
                ListItem(
                    headlineContent = { Text(w.name) },
                    leadingContent = { RadioButton(selected = w.id == ws, onClick = { state.switchWorkspace(w.id) }) },
                    modifier = Modifier.clickable { state.switchWorkspace(w.id) },
                )
            }
        }
        item {
            ListItem(headlineContent = { Text(stringResource(R.string.notifications)) }, leadingContent = { Icon(Icons.Outlined.Notifications, null) }, modifier = Modifier.clickable { opener.route("notifications") })
            val availableUpdate = androidx.compose.ui.platform.LocalContext.current.getSharedPreferences("looty", 0).getString("updateAvailable", null)
            ListItem(
                headlineContent = { Text(stringResource(R.string.updates)) },
                supportingContent = { Text(if (availableUpdate != null) stringResource(R.string.update_available_body, availableUpdate) else stringResource(R.string.update_installed, io.github.untypequicode.looty.update.Updater.current)) },
                leadingContent = { Icon(Icons.Outlined.SystemUpdate, null) },
                modifier = Modifier.clickable { opener.route("updates") },
            )
        }
        item {
            ListItem(headlineContent = { Text(stringResource(R.string.calendar)) }, leadingContent = { Icon(Icons.Outlined.CalendarMonth, null) }, modifier = Modifier.clickable { opener.route("calendar") })
        }
        item {
            ListItem(
                headlineContent = { Text(stringResource(R.string.mail)) }, leadingContent = { Icon(Icons.Outlined.Mail, null) },
                trailingContent = { if (state.mailUnread > 0) Badge { Text(if (state.mailUnread > 99) "99+" else state.mailUnread.toString()) } },
                modifier = Modifier.clickable { opener.route("mail") },
            )
        }
        item { ListItem(headlineContent = { Text(stringResource(R.string.drive)) }, leadingContent = { Icon(Icons.Outlined.Cloud, null) }, modifier = Modifier.clickable { opener.route("drive") }) }
        item { ListItem(headlineContent = { Text(stringResource(R.string.projects)) }, leadingContent = { Icon(Icons.Outlined.Code, null) }, modifier = Modifier.clickable { opener.route("projects") }) }
        item {
            SectionTitle(stringResource(R.string.boards), action = if (state.canManageBoards) { {
                IconButton(onClick = { newBoard = true }) { Icon(Icons.Outlined.Add, stringResource(R.string.new_board)) }
            } } else null)
        }
        items(state.boards, key = { "b" + it.id }) { b ->
            ListItem(headlineContent = { Text(b.name) }, leadingContent = { Icon(Icons.Outlined.ViewKanban, null) }, modifier = Modifier.clickable { opener.board(b.id) })
        }
        item { SectionTitle(stringResource(R.string.open_web)) }
        items(links, key = { it.third }) { (icon, label, screen) ->
            ListItem(headlineContent = { Text(stringResource(label)) }, leadingContent = { Icon(icon, null) }, modifier = Modifier.clickable { openWeb(ctx, Paths.screen(ws, screen)) })
        }
        item { HorizontalDivider(Modifier.padding(vertical = 8.dp)) }
        item {
            ListItem(headlineContent = { Text(stringResource(R.string.change_server)) }, leadingContent = { Icon(Icons.Outlined.Dns, null) },
                modifier = Modifier.clickable { ctx.startActivity(Intent(ctx, SetupActivity::class.java)) })
        }
        item {
            ListItem(headlineContent = { Text(stringResource(R.string.sign_out)) }, leadingContent = { Icon(Icons.AutoMirrored.Outlined.Logout, null) },
                modifier = Modifier.clickable { state.logout(onSignedOut) })
        }
        item { Text(stringResource(R.string.app_version, BuildConfig.VERSION_NAME), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.outline, modifier = Modifier.padding(20.dp)) }
    }
}
