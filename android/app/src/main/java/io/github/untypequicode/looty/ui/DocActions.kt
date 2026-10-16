package io.github.untypequicode.looty.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.widget.Toast
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.Article
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Page
import io.github.untypequicode.looty.api.PagePerm
import kotlinx.coroutines.launch

/** Ce que la personne peut faire sur un document (règles du serveur). */
fun AppState.canWritePage(p: Page) = p.can_edit == true
fun AppState.canSharePage(p: Page) = canWritePage(p) && access?.permissions?.share == true
fun AppState.canAddSubpage(p: Page) = (canWritePage(p) && canEditDocs) || p.can_add_child == true

/**
 * Actions d'un document : ouvrir, sous-page, renommer, icône, favori, déplacer, partager,
 * commentaires, versions, corbeille. Les boîtes de dialogue sont gérées ici.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DocActionsSheet(
    state: AppState, page: Page, onDismiss: () -> Unit, onOpen: (() -> Unit)?, onComments: () -> Unit, onHistory: () -> Unit, onDeleted: (Page) -> Unit,
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var dialog by remember { mutableStateOf<String?>(null) }
    val write = state.canWritePage(page)
    if (dialog == null) ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
            Row(Modifier.padding(horizontal = 24.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                DocIcon(page, 24.dp)
                Spacer(Modifier.width(12.dp))
                Text(page.title.ifBlank { stringResource(R.string.untitled) }, style = MaterialTheme.typography.titleMedium, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
            @Composable
            fun item(icon: androidx.compose.ui.graphics.vector.ImageVector, label: Int, danger: Boolean = false, onClick: () -> Unit) = ListItem(
                colors = sheetItemColors(),
                headlineContent = { Text(stringResource(label), color = if (danger) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified) },
                leadingContent = { Icon(icon, null, tint = if (danger) MaterialTheme.colorScheme.error else LocalContentColor.current) },
                modifier = Modifier.clickable(onClick = onClick),
            )
            onOpen?.let { item(Icons.AutoMirrored.Outlined.Article, R.string.open) { onDismiss(); it() } }
            if (write) item(Icons.Outlined.Edit, R.string.edit_in_editor) { onDismiss(); state.ws?.let { openWeb(ctx, Paths.doc(it, page.id)) } }
            if (state.canAddSubpage(page)) item(Icons.Outlined.NoteAdd, R.string.new_subpage) {
                onDismiss()
                state.launch { state.createPage(parentId = page.id).onSuccess { id -> state.ws?.let { openWeb(ctx, Paths.doc(it, id)) } }.onFailure { state.lastError = (it as? io.github.untypequicode.looty.api.ApiException)?.code ?: "offline" } }
            }
            item(if (page.is_favorite == true) Icons.Outlined.StarOutline else Icons.Outlined.Star, if (page.is_favorite == true) R.string.remove_favorite else R.string.add_favorite) { onDismiss(); state.toggleFavorite(page) }
            if (write) {
                item(Icons.Outlined.DriveFileRenameOutline, R.string.rename) { dialog = "rename" }
                item(Icons.Outlined.EmojiEmotions, R.string.change_icon) { dialog = "icon" }
                item(Icons.Outlined.DriveFileMove, R.string.move_to_folder) { dialog = "move" }
            }
            item(Icons.Outlined.ChatBubbleOutline, R.string.comments) { onDismiss(); onComments() }
            item(Icons.Outlined.History, R.string.version_history) { onDismiss(); onHistory() }
            if (state.canSharePage(page) || state.isAdmin) item(Icons.Outlined.Share, R.string.share) { dialog = "share" }
            if (write) item(Icons.Outlined.Delete, R.string.delete, danger = true) { dialog = "delete" }
        }
    }
    when (dialog) {
        "rename" -> NameDialog(stringResource(R.string.rename), stringResource(R.string.doc_title), page.title, onDismiss = onDismiss) { state.renamePage(page, it); onDismiss() }
        "icon" -> IconDialog(page.icon, onDismiss = onDismiss) { state.setPageIcon(page, it); onDismiss() }
        "move" -> MovePageSheet(state, page, onDismiss = onDismiss) { state.movePage(page, it); onDismiss() }
        "share" -> ShareSheet(state, page, onDismiss = onDismiss)
        "delete" -> AlertDialog(
            onDismissRequest = onDismiss,
            title = { Text(stringResource(R.string.delete_doc_title, page.title.ifBlank { stringResource(R.string.untitled) })) },
            text = { Text(stringResource(R.string.delete_doc_message)) },
            confirmButton = { TextButton(onClick = { onDismiss(); state.launch { if (state.deletePage(page)) onDeleted(page) } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** Icône d'un document (emoji) ou icône par défaut. */
@Composable
fun DocIcon(p: Page, size: androidx.compose.ui.unit.Dp = 22.dp) {
    if (!p.icon.isNullOrBlank()) Text(p.icon, fontSize = (size.value * 0.8f).sp, modifier = Modifier.widthIn(min = size))
    else Icon(Icons.AutoMirrored.Outlined.Article, null, Modifier.size(size), tint = MaterialTheme.colorScheme.onSurfaceVariant)
}

private val EMOJIS = listOf("📄", "📝", "📌", "📎", "📚", "📖", "📒", "📓", "🗂️", "📁", "🗓️", "📅", "✅", "☑️", "⭐", "💡", "🎯", "🚀", "🔥", "⚡",
    "🧪", "🔬", "🧮", "📊", "📈", "💼", "🏠", "🎓", "🏫", "🧠", "💬", "📣", "🔒", "🔑", "🛠️", "⚙️", "🎨", "🎵", "🌍", "❤️",
    "😀", "🙂", "🤔", "👋", "👍", "🙏", "🐞", "🧩", "📦", "🗒️")

/** Choix d'une icône (emoji) pour un document, ou aucune. */
@Composable
private fun IconDialog(current: String?, onDismiss: () -> Unit, onPick: (String?) -> Unit) {
    var custom by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.change_icon)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                LazyVerticalGrid(GridCells.Adaptive(44.dp), Modifier.heightIn(max = 260.dp)) {
                    items(EMOJIS) { e ->
                        Box(Modifier.size(44.dp).clip(RoundedCornerShape(8.dp)).clickable { onPick(e) }.then(
                            if (e == current) Modifier.padding(2.dp) else Modifier), contentAlignment = Alignment.Center) { Text(e, fontSize = 24.sp) }
                    }
                }
                // Autre emoji : celui du clavier.
                OutlinedTextField(custom, { custom = it.take(8) }, singleLine = true, label = { Text(stringResource(R.string.other_emoji)) }, modifier = Modifier.fillMaxWidth())
            }
        },
        confirmButton = { TextButton(enabled = custom.isNotBlank(), onClick = { onPick(custom.trim()) }) { Text(stringResource(R.string.save)) } },
        dismissButton = {
            Row {
                if (!current.isNullOrBlank()) TextButton(onClick = { onPick(null) }) { Text(stringResource(R.string.remove_icon)) }
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
            }
        },
    )
}

/** Nouvel emplacement d'un document : à la racine ou sous un autre (pas sous lui-même ni ses sous-pages). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MovePageSheet(state: AppState, page: Page, onDismiss: () -> Unit, onPick: (String?) -> Unit) {
    val all = state.pages.data.orEmpty()
    val byParent = remember(all) { all.groupBy { it.parent_id }.mapValues { (_, v) -> v.sortedBy { it.position } } }
    val rows = remember(all) {
        buildList {
            fun walk(parent: String?, depth: Int) {
                for (p in byParent[parent].orEmpty()) {
                    if (p.id == page.id) continue // ni lui-même, ni ses sous-pages
                    if (p.can_edit == true) add(p to depth)
                    walk(p.id, depth + 1)
                }
            }
            walk(null, 0)
        }
    }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
            Text(stringResource(R.string.move_to_folder), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            if (state.canEditDocs) ListItem(colors = sheetItemColors(), headlineContent = { Text(stringResource(R.string.top_level)) },
                leadingContent = { Icon(Icons.Outlined.VerticalAlignTop, null) }, trailingContent = { if (page.parent_id == null) Icon(Icons.Outlined.Check, null) },
                modifier = Modifier.clickable(enabled = page.parent_id != null) { onPick(null) })
            rows.forEach { (p, depth) ->
                ListItem(colors = sheetItemColors(), headlineContent = { Text(p.title.ifBlank { stringResource(R.string.untitled) }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    leadingContent = { DocIcon(p) }, trailingContent = { if (page.parent_id == p.id) Icon(Icons.Outlined.Check, null) },
                    modifier = Modifier.padding(start = (depth * 20).dp).clickable(enabled = page.parent_id != p.id) { onPick(p.id) })
            }
        }
    }
}

/** Partage : lien public (lecture sans compte) et accès particuliers des personnes (admins). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ShareSheet(state: AppState, page0: Page, onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val page = state.pages.data?.firstOrNull { it.id == page0.id } ?: page0
    var busy by remember { mutableStateOf(false) }
    var perms by remember { mutableStateOf<List<PagePerm>?>(null) }
    var adding by remember { mutableStateOf(false) }
    val link = page.share_token?.takeIf { page.is_public == true }?.let { t -> runCatching { state.api.server }.getOrNull()?.let { "$it/public/$t" } }
    val copied = stringResource(R.string.link_copied)
    LaunchedEffect(page.id) { if (state.isAdmin) perms = state.pagePermissions(page.id) }
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().padding(bottom = 16.dp)) {
            Text(stringResource(R.string.share), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            if (state.canSharePage(page)) {
                ListItem(colors = sheetItemColors(),
                    headlineContent = { Text(stringResource(R.string.public_link)) },
                    supportingContent = { Text(stringResource(R.string.public_link_hint)) },
                    leadingContent = { Icon(Icons.Outlined.Public, null) },
                    trailingContent = { Switch(checked = link != null, enabled = !busy, onCheckedChange = { on -> busy = true; scope.launch { state.setPagePublic(page.id, on); busy = false } }) })
                link?.let { url ->
                    Text(url, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.primary, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(horizontal = 24.dp))
                    Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedButton(onClick = {
                            ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(page.title, url))
                            Toast.makeText(ctx, copied, Toast.LENGTH_SHORT).show()
                        }) { Icon(Icons.Outlined.ContentCopy, null, Modifier.size(18.dp)); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.copy_link)) }
                        Button(onClick = {
                            ctx.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_SUBJECT, page.title).putExtra(Intent.EXTRA_TEXT, url), null))
                        }) { Icon(Icons.Outlined.Share, null, Modifier.size(18.dp)); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.send_link)) }
                    }
                }
            }
            if (state.isAdmin) {
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                SectionTitle(stringResource(R.string.special_access), action = { TextButton(onClick = { adding = true }) { Icon(Icons.Outlined.PersonAdd, null, Modifier.size(18.dp)); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.add_person)) } })
                Text(stringResource(R.string.special_access_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp))
                val list = perms
                if (list == null) Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp)) }
                else if (list.isEmpty()) Text(stringResource(R.string.no_special_access), style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(16.dp))
                list?.forEach { p ->
                    PermRow(p, onChange = { role, inherit ->
                        scope.launch { if (state.setPagePermission(page.id, p.user_id, role, inherit, p.allow_subpages && role == "editor")) perms = state.pagePermissions(page.id) }
                    })
                }
            }
        }
    }
    if (adding) {
        val taken = perms.orEmpty().map { it.user_id }.toSet()
        AddPersonSheet(state, exclude = taken, onDismiss = { adding = false }) { userId, role ->
            adding = false
            scope.launch { if (state.setPagePermission(page.id, userId, role, true, false)) perms = state.pagePermissions(page.id) }
        }
    }
}

private val ROLES = listOf("viewer" to R.string.role_viewer, "editor" to R.string.role_editor, "blocked" to R.string.role_blocked)

@Composable
private fun PermRow(p: PagePerm, onChange: (role: String, inherit: Boolean) -> Unit) {
    var menu by remember { mutableStateOf(false) }
    ListItem(
        headlineContent = { Text(p.name.ifBlank { p.email }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = {
            Text(p.inherited_from?.let { stringResource(R.string.inherited_from, it.title.ifBlank { stringResource(R.string.untitled) }) }
                ?: stringResource(if (p.inherit) R.string.with_subpages else R.string.this_page_only), maxLines = 1)
        },
        leadingContent = { Avatar(p.name.ifBlank { p.email }, 36.dp) },
        trailingContent = {
            Box {
                TextButton(enabled = p.inherited_from == null, onClick = { menu = true }) { Text(stringResource(ROLES.firstOrNull { it.first == p.role }?.second ?: R.string.role_viewer)); Icon(Icons.Outlined.ArrowDropDown, null) }
                DropdownMenu(menu, { menu = false }) {
                    ROLES.filter { p.is_member || it.first != "blocked" }.forEach { (r, label) ->
                        DropdownMenuItem(text = { Text(stringResource(label)) }, leadingIcon = { RadioButton(p.role == r, null) }, onClick = { menu = false; onChange(r, p.inherit) })
                    }
                    HorizontalDivider()
                    DropdownMenuItem(text = { Text(stringResource(R.string.with_subpages)) }, leadingIcon = { Checkbox(p.inherit, null) }, onClick = { menu = false; onChange(p.role, !p.inherit) })
                    DropdownMenuItem(text = { Text(stringResource(R.string.remove), color = MaterialTheme.colorScheme.error) }, onClick = { menu = false; onChange("none", p.inherit) })
                }
            }
        },
    )
}

/** Choix d'un membre de l'espace et de son accès. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddPersonSheet(state: AppState, exclude: Set<String>, onDismiss: () -> Unit, onAdd: (String, String) -> Unit) {
    var role by remember { mutableStateOf("viewer") }
    var query by remember { mutableStateOf("") }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().imePadding().padding(bottom = 16.dp)) {
            Text(stringResource(R.string.add_person), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
                ROLES.forEachIndexed { i, (r, label) ->
                    SegmentedButton(selected = role == r, onClick = { role = r }, shape = SegmentedButtonDefaults.itemShape(i, ROLES.size)) { Text(stringResource(label), maxLines = 1) }
                }
            }
            OutlinedTextField(query, { query = it }, singleLine = true, leadingIcon = { Icon(Icons.Outlined.Search, null) }, placeholder = { Text(stringResource(R.string.people)) },
                modifier = Modifier.fillMaxWidth().padding(16.dp))
            state.members.values.filter { it.user_id !in exclude && (query.isBlank() || it.name.contains(query, true) || it.email.contains(query, true)) }.sortedBy { it.name }.forEach { m ->
                ListItem(colors = sheetItemColors(), headlineContent = { Text(m.name) }, supportingContent = { Text(m.email) }, leadingContent = { Avatar(m.name, 32.dp) },
                    modifier = Modifier.clickable { onAdd(m.user_id, role) })
            }
        }
    }
}
