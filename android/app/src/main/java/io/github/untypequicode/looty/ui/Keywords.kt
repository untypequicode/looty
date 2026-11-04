package io.github.untypequicode.looty.ui

import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Tag
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.KeywordUse
import kotlinx.coroutines.launch

private fun fold(s: String) = java.text.Normalizer.normalize(s.lowercase(), java.text.Normalizer.Form.NFD).replace(Regex("\\p{Mn}+"), "")

/**
 * Mots-clés d'un élément (GLOSSARY) : puces, et ajout avec ceux déjà employés dans l'espace.
 * `kind` : page, task, board, project, file, event.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun KeywordsRow(state: AppState, kind: String, id: String, modifier: Modifier = Modifier) {
    val scope = rememberCoroutineScope()
    var keywords by remember(id) { mutableStateOf<List<String>>(emptyList()) }
    var canEdit by remember(id) { mutableStateOf(false) }
    var adding by remember { mutableStateOf(false) }
    LaunchedEffect(kind, id) { state.keywords(kind, id)?.let { keywords = it.keywords; canEdit = it.can_edit } }
    fun save(next: List<String>) = scope.launch { state.setKeywords(kind, id, next)?.let { keywords = it.keywords } }
    FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (keywords.isEmpty() && !canEdit) Text("—")
        keywords.forEach { k -> SuggestionChip(onClick = { if (canEdit) adding = true }, label = { Text("#$k") }) }
        if (canEdit) AssistChip(onClick = { adding = true }, label = { Text(stringResource(R.string.keywords_add)) }, leadingIcon = { Icon(Icons.Outlined.Tag, null, Modifier.size(16.dp)) })
    }
    // Ajouter et retirer : dans la feuille (une puce touchée l'ouvre aussi).
    if (adding) KeywordsSheet(state, keywords, onDismiss = { adding = false }, onChange = { save(it) })
}

/** Feuille d'édition : mots-clés actuels (à retirer) et suggestions de l'espace. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun KeywordsSheet(state: AppState, current: List<String>, onDismiss: () -> Unit, onChange: (List<String>) -> Unit) {
    var known by remember { mutableStateOf<List<KeywordUse>>(emptyList()) }
    var list by remember { mutableStateOf(current) }
    var text by remember { mutableStateOf("") }
    LaunchedEffect(Unit) { known = state.knownKeywords() }
    fun set(next: List<String>) { list = next; onChange(next) }
    fun add(raw: String) {
        val k = raw.trim().trimStart('#').replace(Regex("\\s+"), " ").take(50)
        if (k.isNotEmpty() && list.none { fold(it) == fold(k) } && list.size < 20) set(list + k)
        text = ""
    }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp).imePadding(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(stringResource(R.string.keywords), style = MaterialTheme.typography.titleLarge)
            Text(stringResource(R.string.keywords_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                list.forEach { k ->
                    InputChip(selected = true, onClick = { set(list - k) }, label = { Text("#$k") }, trailingIcon = { Icon(Icons.Outlined.Close, stringResource(R.string.clear), Modifier.size(16.dp)) })
                }
            }
            OutlinedTextField(text, { if (it.endsWith(",") || it.endsWith("\n")) add(it.dropLast(1)) else text = it }, singleLine = true,
                label = { Text(stringResource(R.string.keyword_new)) }, modifier = Modifier.fillMaxWidth(),
                trailingIcon = { TextButton(enabled = text.isNotBlank(), onClick = { add(text) }) { Text(stringResource(R.string.add)) } })
            val q = fold(text.trim())
            val suggestions = known.filter { s -> list.none { fold(it) == fold(s.keyword) } && (q.isEmpty() || fold(s.keyword).contains(q)) }.take(20)
            if (suggestions.isNotEmpty()) FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                suggestions.forEach { s -> SuggestionChip(onClick = { add(s.keyword) }, label = { Text("#${s.keyword}") }) }
            }
        }
    }
}
