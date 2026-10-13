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
    val refused = state.refused
    LaunchedEffect(refused) {
        if (refused.isNotEmpty()) {
            state.refused = emptyList()
            snackbar.showSnackbar(refusedMsg + " " + refused.take(3).joinToString(", ") { "« " + it + " »" } + if (refused.size > 3) "…" else "")
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
        if (seg.firstOrNull() != "w" || ws == null) return@LaunchedEffect openWeb(ctx, link)
        if (ws != state.ws) state.switchWorkspace(ws)
        val target = when {
            seg.getOrNull(2) == "task" && seg.size > 3 -> "task/${seg[3]}"
            seg.getOrNull(2) == "board" && uri.getQueryParameter("task") != null -> "task/${uri.getQueryParameter("task")}"
            seg.getOrNull(2) == "board" && seg.size > 3 -> "board/${seg[3]}"
            seg.getOrNull(2) == "doc" && seg.size > 3 -> "doc/${seg[3]}"
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
            composable("docs") { DocsScreen(state, padding) }
            composable("inbox") { InboxScreen(state, padding) }
            composable("more") { MoreScreen(state, padding, onSignedOut) }
            composable("board/{id}") { e -> BoardScreen(state, e.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() }, onTask = { nav.navigate("task/$it") }) }
            composable("doc/{id}") { e -> ReaderScreen(state, e.arguments?.getString("id") ?: "", onBack = { nav.popBackStack() }) }
            composable("notifications") { NotificationsScreen(state, onBack = { nav.popBackStack() }) }
            composable("calendar") { CalendarScreen(state, onBack = { nav.popBackStack() }, onEdit = { id, date -> nav.navigate("event?id=${id.orEmpty()}&date=$date") }) }
            composable("event?id={id}&date={date}") { e ->
                val id = e.arguments?.getString("id")?.ifEmpty { null }
                val date = e.arguments?.getString("date")?.let { runCatching { java.time.LocalDate.parse(it) }.getOrNull() } ?: java.time.LocalDate.now()
                EventEditor(state, id, date, onDone = { nav.popBackStack() })
            }
            composable("mail") { MailScreen(state, onBack = { nav.popBackStack() }, onMessage = { nav.navigate("mail/$it") }) }
            composable("mail/{uid}") { e -> MessageScreen(state, e.arguments?.getString("uid")?.toLongOrNull() ?: 0L, onBack = { nav.popBackStack() }) }
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
