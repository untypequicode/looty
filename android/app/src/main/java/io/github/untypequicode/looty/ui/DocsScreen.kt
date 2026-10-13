package io.github.untypequicode.looty.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Article
import androidx.compose.material.icons.outlined.ChevronRight
import androidx.compose.material.icons.outlined.ExpandMore
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Star
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Page

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DocsScreen(state: AppState, padding: PaddingValues) {
    val ctx = LocalContext.current
    val opener = LocalOpener.current
    val ws = state.ws ?: return
    val all = state.pages.data.orEmpty()
    var query by rememberSaveable { mutableStateOf("") }
    var expanded by rememberSaveable { mutableStateOf(setOf<String>()) }
    val byParent = remember(all) { all.groupBy { it.parent_id }.mapValues { (_, v) -> v.sortedBy { it.position } } }
    // Arbre aplati selon les nœuds dépliés (ou liste filtrée pendant la recherche).
    val rows = remember(all, expanded, query) {
        if (query.isNotBlank()) all.filter { it.title.contains(query.trim(), ignoreCase = true) }.map { it to 0 }
        else buildList {
            fun walk(parent: String?, depth: Int) {
                for (p in byParent[parent].orEmpty()) {
                    add(p to depth)
                    if (p.id in expanded) walk(p.id, depth + 1)
                }
            }
            walk(null, 0)
        }
    }
    val favorites = all.filter { it.is_favorite == true }
    Column(Modifier.padding(padding)) {
        OutlinedTextField(
            value = query, onValueChange = { query = it }, singleLine = true,
            leadingIcon = { Icon(Icons.Outlined.Search, null) }, placeholder = { Text(stringResource(R.string.docs_search)) },
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), shape = MaterialTheme.shapes.extraLarge,
        )
        PullToRefreshBox(isRefreshing = state.pages.loading && state.pages.data != null, onRefresh = { state.loadPages() }) {
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
                if (query.isBlank() && favorites.isNotEmpty()) {
                    item { SectionTitle(stringResource(R.string.favorites)) }
                    items(favorites, key = { "f" + it.id }) { p -> DocRow(p, 0, hasChildren = false, open = false, onToggle = {}) { opener.route("doc/${p.id}") } }
                    item { SectionTitle(stringResource(R.string.nav_docs)) }
                }
                if (rows.isEmpty() && !state.pages.loading) item { EmptyState(Icons.AutoMirrored.Outlined.Article, stringResource(R.string.no_docs)) }
                items(rows, key = { it.first.id }) { (p, depth) ->
                    DocRow(p, depth, hasChildren = query.isBlank() && byParent[p.id].orEmpty().isNotEmpty(), open = p.id in expanded,
                        onToggle = { expanded = if (p.id in expanded) expanded - p.id else expanded + p.id }) { opener.route("doc/${p.id}") }
                }
            }
        }
    }
}

@Composable
private fun DocRow(p: Page, depth: Int, hasChildren: Boolean, open: Boolean, onToggle: () -> Unit, onOpen: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(start = (8 + depth * 20).dp, end = 16.dp).heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (hasChildren) IconButton(onClick = onToggle) { Icon(if (open) Icons.Outlined.ExpandMore else Icons.Outlined.ChevronRight, null) }
        else Spacer(Modifier.width(48.dp))
        if (!p.icon.isNullOrBlank()) Text(p.icon, modifier = Modifier.width(28.dp)) else Icon(if (p.is_favorite == true) Icons.Outlined.Star else Icons.AutoMirrored.Outlined.Article, null, Modifier.width(28.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(p.title.ifBlank { stringResource(R.string.untitled) }, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge)
    }
}
