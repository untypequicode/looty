package io.github.untypequicode.looty.ui

import androidx.compose.ui.Alignment
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Article
import androidx.compose.material.icons.automirrored.outlined.Article
import androidx.compose.material.icons.filled.*
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.filterNotNull
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.Session
import kotlinx.coroutines.launch

private data class Tab(val route: String, val label: Int, val icon: ImageVector, val selected: ImageVector)

private val TABS = listOf(
    Tab("home", R.string.nav_home, Icons.Outlined.Home, Icons.Filled.Home),
    Tab("tasks", R.string.nav_tasks, Icons.Outlined.TaskAlt, Icons.Filled.TaskAlt),
    Tab("docs", R.string.nav_docs, Icons.AutoMirrored.Outlined.Article, Icons.AutoMirrored.Filled.Article),
    Tab("inbox", R.string.nav_inbox, Icons.Outlined.Inbox, Icons.Filled.Inbox),
    Tab("more", R.string.nav_more, Icons.Outlined.Menu, Icons.Filled.Menu),
)

/** Écrans natifs : barre de navigation du bas, une barre supérieure par écran. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LootyApp(state: AppState, onSignedOut: () -> Unit) {
    val ctx = LocalContext.current
    val nav = rememberNavController()
    val entry by nav.currentBackStackEntryAsState()
    val route = entry?.destination?.route ?: "home"
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    var quickAdd by remember { mutableStateOf(false) }
    val createdMsg = stringResource(R.string.created)
    val refusedMsg = stringResource(R.string.sync_refused)
    // Modifications faites hors ligne que le serveur a refusées au rejeu : signalées une fois, par nom.
    // (snapshotFlow : vider la valeur ne doit pas annuler le message en cours d'affichage.)
    LaunchedEffect(Unit) {
        snapshotFlow { state.refused }.filter { it.isNotEmpty() }.collect { refused ->
            state.refused = emptyList()
            snackbar.showSnackbar(refusedMsg + " " + refused.take(3).joinToString(", ") { "« " + it + " »" } + if (refused.size > 3) "…" else "")
        }
    }
    // Écriture refusée ou impossible (droits, serveur injoignable) : dite une fois.
    LaunchedEffect(Unit) {
        snapshotFlow { state.lastError }.filterNotNull().collect { e ->
            state.lastError = null
            snackbar.showSnackbar(errorText(ctx, e))
        }
    }
    val isTab = TABS.any { it.route == route }
    val opener = remember(nav) { Opener(task = { nav.navigate("task/$it") }, board = { nav.navigate("board/$it") }, route = { nav.navigate(it) { launchSingleTop = true } }) }
    val workspace = (state.session as? Session.Ready)?.let { s -> s.workspaces.firstOrNull { it.id == s.workspaceId }?.name }.orEmpty()

    // Lien reçu (notification, « Ouvrir dans l'application ») : écran natif si possible, sinon la page web.
    val link = state.pendingLink
    LaunchedEffect(link) {
        if (link == null) return@LaunchedEffect
        state.pendingLink = null
        val uri = android.net.Uri.parse(link)
        val seg = uri.pathSegments
        val ws = seg.getOrNull(1)
        if (seg.firstOrNull() == "app" && seg.getOrNull(1) == "updates") { nav.navigate("updates") { launchSingleTop = true }; return@LaunchedEffect }
        // Widgets : ajout rapide et écrans principaux (looty://app/<écran>).
        if (seg.firstOrNull() == "app") {
            when (val screen = seg.getOrNull(1)) {
                "quickadd" -> quickAdd = true
                "tasks", "docs", "inbox", "home" -> nav.navigate(screen) { launchSingleTop = true }
                "calendar", "mail", "moodle", "drive", "projects", "notifications" -> nav.navigate(screen) { launchSingleTop = true }
            }
            return@LaunchedEffect
        }
        if (seg.firstOrNull() != "w" || ws == null) return@LaunchedEffect openWeb(ctx, link)
        if (ws != state.ws) state.switchWorkspace(ws)
        val target = when {
            seg.getOrNull(2) == "task" && seg.size > 3 -> "task/${seg[3]}"
            seg.getOrNull(2) == "board" && uri.getQueryParameter("task") != null -> "task/${uri.getQueryParameter("task")}"
            seg.getOrNull(2) == "board" && seg.size > 3 -> "board/${seg[3]}"
            seg.getOrNull(2) == "doc" && seg.size > 3 && uri.getQueryParameter("comment") != null -> "doc/${seg[3]}/comments"
            seg.getOrNull(2) == "doc" && seg.size > 3 -> "doc/${seg[3]}"
            seg.getOrNull(2) == "drive" -> "drive"
            seg.getOrNull(2) == "settings" && seg.getOrNull(3) == "notifications" -> "notifications"
            seg.getOrNull(2) == "settings" && seg.getOrNull(3) in setOf("profile", "preferences", "security", "calendars", "workspace", "members") -> "settings/${seg[3]}"
            seg.getOrNull(2) == "settings" && seg.size == 3 -> "settings"
            seg.getOrNull(2) == "moodle" -> "moodle"
            seg.getOrNull(2) == "activity" -> "activity"
            seg.getOrNull(2) == "trash" -> "trash"
            seg.getOrNull(2) == "graph" -> "graph"
            seg.getOrNull(2) == "projects" && seg.size > 3 -> "project/${seg[3]}"
            seg.getOrNull(2) == "projects" -> "projects"
            seg.getOrNull(2) == "inbox" -> "inbox"
            seg.getOrNull(2) == "calendar" -> "calendar"
            seg.getOrNull(2) == "mail" && seg.size == 3 -> "mail"
            else -> null
        }
        if (target != null) nav.navigate(target) { launchSingleTop = true } else openWeb(ctx, link)
    }

    CompositionLocalProvider(LocalOpener provides opener) {
    Scaffold(
        topBar = {
            if (isTab) TopAppBar(
                title = { Text(if (route == "home") workspace else stringResource(TABS.firstOrNull { it.route == route }?.label ?: R.string.nav_home)) },
                actions = {
                    when (route) {
                        "inbox" -> if (state.unread > 0) TextButton(onClick = { state.markAllRead() }) { Text(stringResource(R.string.mark_all_read)) }
                        "docs" -> if (state.canEditDocs) IconButton(onClick = {
                            scope.launch { state.createPage().onSuccess { id -> state.ws?.let { openWeb(ctx, Paths.doc(it, id)) } } }
                        }) { Icon(Icons.Outlined.NoteAdd, stringResource(R.string.new_doc)) }
                    }
                },
            )
        },
        bottomBar = {
            Column {
            OfflineBanner(state, padNav = !isTab)
            if (isTab) NavigationBar {
                TABS.forEach { tab ->
                    val selected = route == tab.route
                    NavigationBarItem(
                        selected = selected,
                        onClick = {
                            nav.navigate(tab.route) {
                                popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                                launchSingleTop = true
                                restoreState = true
                            }
                        },
                        icon = {
                            BadgedBox(badge = { if (tab.route == "inbox" && state.unread > 0) Badge { Text(if (state.unread > 99) "99+" else state.unread.toString()) } }) {
                                Icon(if (selected) tab.selected else tab.icon, null)
                            }
                        },
                        label = { Text(stringResource(tab.label)) },
                    )
                }
            }
            }
        },
        floatingActionButton = {
            if ((route == "home" || route == "tasks") && state.canEditBoards) {
                ExtendedFloatingActionButton(onClick = { quickAdd = true }, icon = { Icon(Icons.Filled.Add, null) }, text = { Text(stringResource(R.string.new_task)) })
            }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { innerPadding ->
        // Écrans de détail : leur propre barre supérieure, sans barre de navigation du bas.
        val padding = if (isTab) innerPadding else PaddingValues()
        NavHost(nav, startDestination = "home", modifier = if (isTab) Modifier else Modifier.padding(bottom = innerPadding.calculateBottomPadding())) {
            composable("home") { HomeScreen(state, padding) { nav.navigate("tasks") { launchSingleTop = true } } }
            composable("tasks") { TasksScreen(state, padding, snackbar) }
            composable("docs") { DocsScreen(state, padding, snackbar) }
            composable("inbox") { InboxScreen(state, padding) }
            composable("more") { MoreScreen(state, padding, onSignedOut) }
            composable("board/{id}") { e ->
                val id = e.arguments?.getString("id") ?: ""
                BoardScreen(state, id, onBack = { nav.popBackStack() }, onTask = { nav.navigate("task/$it") }, onSettings = { nav.navigate("board/$id/settings") })
            }
            composable("board/{id}/settings") { e ->
                BoardSettingsScreen(state, e.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() },
                    onDeleted = { nav.popBackStack("board/{id}", inclusive = true) })
            }
            composable("doc/{id}") { e -> ReaderScreen(state, e.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() }, onRoute = { nav.navigate(it) }) }
            composable("doc/{id}/comments") { e -> DocCommentsScreen(state, e.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() }) }
            composable("doc/{id}/history") { e -> DocHistoryScreen(state, e.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() }) }
            composable("notifications") { NotificationsScreen(state, onBack = { nav.popBackStack() }) }
            composable("updates") { UpdatesScreen(state, onBack = { nav.popBackStack() }) }
            composable("calendar") { CalendarScreen(state, onBack = { nav.popBackStack() }, onEdit = { id, date -> nav.navigate("event?id=${id.orEmpty()}&date=$date") }) }
            composable("event?id={id}&date={date}") { e ->
                val id = e.arguments?.getString("id")?.ifEmpty { null }
                val date = e.arguments?.getString("date")?.let { runCatching { java.time.LocalDate.parse(it) }.getOrNull() } ?: java.time.LocalDate.now()
                EventEditor(state, id, date, onDone = { nav.popBackStack() })
            }
            composable("mail") {
                MailScreen(state, onBack = { nav.popBackStack() },
                    onMessage = { uid, folder -> nav.navigate("mail/$uid?folder=" + android.net.Uri.encode(folder)) },
                    onCompose = { nav.navigate("mailcompose?mode=new") },
                    onAccounts = { id -> if (id == null) nav.navigate("mailaccounts") else nav.navigate("mailaccount?id=$id") })
            }
            composable("mail/{uid}?folder={folder}") { e ->
                MessageScreen(state, e.arguments?.getString("uid")?.toLongOrNull() ?: 0L, e.arguments?.getString("folder") ?: "INBOX", onBack = { nav.popBackStack() },
                    onCompose = { mode -> nav.navigate("mailcompose?mode=$mode") })
            }
            composable("mailcompose?mode={mode}&to={to}") { e ->
                ComposeScreen(state, e.arguments?.getString("mode") ?: "new", e.arguments?.getString("to").orEmpty(), onClose = { nav.popBackStack() },
                    onSetupSending = { id -> nav.navigate("mailaccount?id=$id") })
            }
            composable("moodle") { MoodleScreen(state, onBack = { nav.popBackStack() }, onConversation = { id, name -> nav.navigate("moodle/$id?name=" + android.net.Uri.encode(name)) }, onCourse = { id, name -> nav.navigate("moodle/course/$id?name=" + android.net.Uri.encode(name)) }) }
            composable("moodle/course/{id}?name={name}") { e -> MoodleCourseScreen(state, e.arguments?.getString("id")?.toLongOrNull() ?: 0L, e.arguments?.getString("name").orEmpty(), onBack = { nav.popBackStack() }) }
            composable("moodle/{id}?name={name}") { e -> MoodleConversationScreen(state, e.arguments?.getString("id")?.toLongOrNull() ?: 0L, e.arguments?.getString("name").orEmpty(), onBack = { nav.popBackStack() }) }
            composable("activity") { ActivityScreen(state, onBack = { nav.popBackStack() }, onRoute = { nav.navigate(it) }) }
            composable("trash") { TrashScreen(state, onBack = { nav.popBackStack() }) }
            composable("graph") { GraphScreen(state, onBack = { nav.popBackStack() }, onRoute = { nav.navigate(it) }) }
            composable("settings") { SettingsScreen(state, onBack = { nav.popBackStack() }, onRoute = { nav.navigate(it) }) }
            composable("settings/profile") { ProfileSettingsScreen(state, onBack = { nav.popBackStack() }) }
            composable("settings/preferences") { PreferencesScreen(state, onBack = { nav.popBackStack() }) }
            composable("widgets") { WidgetsScreen(onBack = { nav.popBackStack() }) }
            composable("settings/security") { SecurityScreen(state, onBack = { nav.popBackStack() }) }
            composable("settings/calendars") { CalendarSyncScreen(state, onBack = { nav.popBackStack() }) }
            composable("settings/workspace") { WorkspaceSettingsScreen(state, onBack = { nav.popBackStack() }, onLeft = { nav.popBackStack("home", inclusive = false) }) }
            composable("settings/members") { MembersScreen(state, onBack = { nav.popBackStack() }) }
            composable("drive") { DriveScreen(state, onBack = { nav.popBackStack() }) }
            composable("projects") { ProjectsScreen(state, onBack = { nav.popBackStack() }, onOpen = { nav.navigate("project/$it") }) }
            composable("project/{id}") { e -> val id = e.arguments?.getString("id") ?: ""; ProjectScreen(state, id, onBack = { nav.popBackStack() }, onFile = { nav.navigate("project/$id/file/$it") }) }
            composable("project/{id}/file/{fid}") { e -> ProjectFileScreen(state, e.arguments?.getString("id") ?: "", e.arguments?.getString("fid") ?: "", onBack = { nav.popBackStack() }) }
            composable("mailaccounts") { MailAccountsScreen(state, onBack = { nav.popBackStack() }, onEdit = { id -> nav.navigate("mailaccount?id=$id") }) }
            composable("mailaccount?id={id}") { e -> MailAccountScreen(state, e.arguments?.getString("id").orEmpty(), onDone = { nav.popBackStack() }) }
            composable("task/{id}") { e -> TaskScreen(state, e.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() }, onTask = { nav.navigate("task/$it") }) }
        }
    }
    }
    if (quickAdd) {
        QuickAddSheet(state, onDismiss = { quickAdd = false }, onCreated = {
            quickAdd = false
            scope.launch { snackbar.showSnackbar(createdMsg) }
        })
    }
}

/** Bandeau « hors ligne » : données gardées sur l'appareil, modifications en attente, bouton pour réessayer. */
@Composable
private fun OfflineBanner(state: AppState, padNav: Boolean) {
    val show = state.offline || state.pendingChanges > 0
    androidx.compose.animation.AnimatedVisibility(show) {
        val scope = rememberCoroutineScope()
        Surface(color = MaterialTheme.colorScheme.tertiaryContainer, modifier = Modifier.fillMaxWidth()) {
            Row((if (padNav) Modifier.windowInsetsPadding(WindowInsets.navigationBars) else Modifier).padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(if (state.offline) Icons.Outlined.CloudOff else Icons.Outlined.Sync, null, tint = MaterialTheme.colorScheme.onTertiaryContainer)
                Spacer(Modifier.width(10.dp))
                Column(Modifier.weight(1f)) {
                    Text(stringResource(if (state.offline) R.string.offline_banner else R.string.syncing_banner), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onTertiaryContainer)
                    if (state.pendingChanges > 0) Text(androidx.compose.ui.res.pluralStringResource(R.plurals.pending_changes, state.pendingChanges, state.pendingChanges), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onTertiaryContainer)
                }
                TextButton(onClick = { scope.launch { state.retryNow() } }) { Text(stringResource(R.string.retry)) }
            }
        }
    }
}
