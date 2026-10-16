package io.github.untypequicode.looty.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Article
import androidx.compose.material.icons.outlined.ChatBubbleOutline
import androidx.compose.material.icons.outlined.ChevronRight
import androidx.compose.material.icons.outlined.ExpandMore
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Page
import kotlinx.coroutines.launch

/** Documents : favoris, récents, arborescence ; appui long (ou ⋮) : actions du document. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DocsScreen(state: AppState, padding: PaddingValues, snackbar: SnackbarHostState) {
    val opener = LocalOpener.current
    val scope = rememberCoroutineScope()
    state.ws ?: return
    val all = state.pages.data.orEmpty()
    var query by rememberSaveable { mutableStateOf("") }
    var expanded by rememberSaveable { mutableStateOf(setOf<String>()) }
    var actionsFor by remember { mutableStateOf<Page?>(null) }
    val deletedMsg = stringResource(R.string.doc_deleted)
    val undo = stringResource(R.string.undo)
    LaunchedEffect(Unit) { state.loadDocCommentCounts() }
    val byParent = remember(all) { all.groupBy { it.parent_id }.mapValues { (_, v) -> v.sortedBy { it.position } } }
    val ids = remember(all) { all.map { it.id }.toSet() }
    // Arbre aplati selon les nœuds dépliés (ou liste filtrée pendant la recherche). Une page dont le
    // parent n'est pas visible (droits) est affichée à la racine.
    val rows = remember(all, expanded, query) {
        if (query.isNotBlank()) all.filter { it.title.contains(query.trim(), ignoreCase = true) }.map { it to 0 }
        else buildList {
            fun walk(parent: String?, depth: Int) {
                val children = if (parent == null) all.filter { it.parent_id == null || it.parent_id !in ids }.sortedBy { it.position } else byParent[parent].orEmpty()
                for (p in children) {
                    add(p to depth)
                    if (p.id in expanded) walk(p.id, depth + 1)
                }
            }
            walk(null, 0)
        }
    }
    val favorites = all.filter { it.is_favorite == true }
    val recent = all.filter { it.last_viewed_at != null }.sortedByDescending { it.last_viewed_at }.take(5)
    fun open(p: Page) = opener.route("doc/${p.id}")
    Column(Modifier.padding(padding)) {
        OutlinedTextField(
            value = query, onValueChange = { query = it }, singleLine = true,
            leadingIcon = { Icon(Icons.Outlined.Search, null) }, placeholder = { Text(stringResource(R.string.docs_search)) },
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), shape = MaterialTheme.shapes.extraLarge,
        )
        PullToRefreshBox(isRefreshing = state.pages.loading && state.pages.data != null, onRefresh = { state.loadPages(); state.loadDocCommentCounts() }) {
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
                if (query.isBlank() && favorites.isNotEmpty()) {
                    item { SectionTitle(stringResource(R.string.favorites)) }
                    items(favorites, key = { "f" + it.id }) { p -> DocRow(state, p, 0, null, false, {}, { open(p) }, { actionsFor = p }) }
                }
                if (query.isBlank() && recent.isNotEmpty()) {
                    item { SectionTitle(stringResource(R.string.recent)) }
                    items(recent, key = { "r" + it.id }) { p -> DocRow(state, p, 0, null, false, {}, { open(p) }, { actionsFor = p }) }
                }
                if (query.isBlank() && (favorites.isNotEmpty() || recent.isNotEmpty())) item { SectionTitle(stringResource(R.string.nav_docs)) }
                if (rows.isEmpty() && !state.pages.loading) item { EmptyState(Icons.AutoMirrored.Outlined.Article, stringResource(R.string.no_docs)) }
                items(rows, key = { it.first.id }) { (p, depth) ->
                    DocRow(state, p, depth, hasChildren = query.isBlank() && byParent[p.id].orEmpty().isNotEmpty(), open = p.id in expanded,
                        onToggle = { expanded = if (p.id in expanded) expanded - p.id else expanded + p.id }, onOpen = { open(p) }, onMore = { actionsFor = p })
                }
            }
        }
    }
    actionsFor?.let { p ->
        DocActionsSheet(state, p, onDismiss = { actionsFor = null }, onOpen = { open(p) },
            onComments = { opener.route("doc/${p.id}/comments") }, onHistory = { opener.route("doc/${p.id}/history") },
            onDeleted = { d -> scope.launch { if (snackbar.showSnackbar(deletedMsg, actionLabel = undo, duration = SnackbarDuration.Long) == SnackbarResult.ActionPerformed) state.restorePage(d.id) } })
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun DocRow(state: AppState, p: Page, depth: Int, hasChildren: Boolean?, open: Boolean, onToggle: () -> Unit, onOpen: () -> Unit, onMore: () -> Unit) {
    val haptics = LocalHapticFeedback.current
    val comments = state.docCommentCounts[p.id] ?: 0
    Row(
        Modifier.fillMaxWidth().combinedClickable(onClick = onOpen, onLongClick = { haptics.performHapticFeedback(HapticFeedbackType.LongPress); onMore() })
            .padding(start = (8 + depth * 20).dp, end = 4.dp).heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        when (hasChildren) {
            true -> IconButton(onClick = onToggle) { Icon(if (open) Icons.Outlined.ExpandMore else Icons.Outlined.ChevronRight, null) }
            false -> Spacer(Modifier.width(48.dp))
            null -> Spacer(Modifier.width(12.dp))
        }
        DocIcon(p)
        Spacer(Modifier.width(8.dp))
        Text(p.title.ifBlank { stringResource(R.string.untitled) }, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge, modifier = Modifier.weight(1f))
        if (comments > 0) {
            Icon(Icons.Outlined.ChatBubbleOutline, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(" $comments", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        IconButton(onClick = onMore) { Icon(Icons.Outlined.MoreVert, null, tint = MaterialTheme.colorScheme.onSurfaceVariant) }
    }
}
