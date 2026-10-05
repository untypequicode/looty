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
import androidx.compose.runtime.Composable
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

@Composable
fun MoreScreen(state: AppState, padding: PaddingValues, onSignedOut: () -> Unit) {
    val ctx = LocalContext.current
    val s = state.session as? Session.Ready ?: return
    val ws = s.workspaceId
    val links: List<Triple<ImageVector, Int, String>> = listOf(
        Triple(Icons.Outlined.CalendarMonth, R.string.calendar, "calendar"),
        Triple(Icons.Outlined.Mail, R.string.mail, "mail"),
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
        item { SectionTitle(stringResource(R.string.boards)) }
        items(state.boards, key = { "b" + it.id }) { b ->
            ListItem(headlineContent = { Text(b.name) }, leadingContent = { Icon(Icons.Outlined.ViewKanban, null) }, modifier = Modifier.clickable { openWeb(ctx, Paths.board(ws, b.id)) })
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
