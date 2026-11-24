package io.github.untypequicode.looty.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.update.Updater
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.util.Locale

/** Entrée « Nouveautés » : notes d'une version Android publiée. */
private data class News(val version: String, val title: String?, val notes: List<String>)

/**
 * Variante « fdroid » : pas de mise à jour intégrée, seulement la version installée et les nouveautés
 * (notes de version Android annoncées par /api/releases du serveur). Les mises à jour viennent de F-Droid.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UpdatesScreen(state: AppState, onBack: () -> Unit) {
    val lang = Locale.getDefault().language
    var news by remember { mutableStateOf<List<News>?>(null) }
    var failed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        runCatching {
            val root = state.api.json.parseToJsonElement(state.api.raw("GET", "/api/releases")).jsonObject
            root["releases"]!!.jsonArray.map { it.jsonObject }
                .filter { it["platform"]?.jsonPrimitive?.content == "android" }
                .take(5)
                .map { e ->
                    News(
                        e["version"]!!.jsonPrimitive.content,
                        e["title"]?.jsonObject?.let { t -> (t[lang] ?: t["en"])?.jsonPrimitive?.content },
                        (e["notes"]?.jsonObject?.get(lang) ?: e["notes"]?.jsonObject?.get("en"))?.jsonArray?.map { n -> n.jsonPrimitive.content } ?: emptyList(),
                    )
                }
        }.onSuccess { news = it }.onFailure { failed = true }
    }
    Scaffold(topBar = {
        TopAppBar(title = { Text(stringResource(R.string.updates)) }, navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, stringResource(R.string.back)) } })
    }) { padding ->
        Column(Modifier.padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Text(stringResource(R.string.update_installed, Updater.current), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.update_store_managed), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            HorizontalDivider()
            Text(stringResource(R.string.update_whats_new), style = MaterialTheme.typography.titleLarge)
            val list = news
            when {
                list != null -> list.forEach { n ->
                    Text(n.version + (n.title?.let { " — $it" } ?: ""), style = MaterialTheme.typography.titleSmall)
                    n.notes.forEach { Text("• " + it.replace("**", "").replace("`", ""), style = MaterialTheme.typography.bodyMedium) }
                }
                failed -> Text(stringResource(R.string.update_news_unavailable), color = MaterialTheme.colorScheme.onSurfaceVariant)
                else -> Row(verticalAlignment = Alignment.CenterVertically) { LoadingIndicatorCompat() }
            }
        }
    }
}
