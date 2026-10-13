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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.BuildConfig
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.update.Update
import io.github.untypequicode.looty.update.UpdateWorker
import io.github.untypequicode.looty.update.Updater
import kotlinx.coroutines.launch
import java.util.Locale

/** Mises à jour de l'application (sans boutique) : version installée, nouvelle version, installation. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun UpdatesScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val prefs = remember { ctx.getSharedPreferences("looty", 0) }
    var auto by remember { mutableStateOf(prefs.getBoolean("autoUpdateCheck", true)) }
    var update by remember { mutableStateOf<Update?>(null) }
    var checked by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }
    var progress by remember { mutableStateOf<Float?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var needPermission by remember { mutableStateOf(false) }
    var noSource by remember { mutableStateOf(false) }
    val lang = Locale.getDefault().language

    fun check() {
        busy = true; error = null
        scope.launch {
            runCatching { Updater.check(ctx, state.api, lang) }
                .onSuccess { update = it; checked = true; prefs.edit().putString("updateAvailable", it?.version).putLong("updateChecked", System.currentTimeMillis()).apply() }
                .onFailure { noSource = it is io.github.untypequicode.looty.update.NoUpdateSource; if (!noSource) error = it.message ?: "error" }
            busy = false
        }
    }
    LaunchedEffect(Unit) { check() }
    // Retour du réglage « Installer des applications inconnues » : on reprend là où on s'était arrêté.
    var resume by remember { mutableStateOf(0) }
    LaunchedEffect(resume) { if (resume > 0 && Updater.ensureCanInstall(ctx)) needPermission = false }

    fun install(u: Update) {
        if (!Updater.ensureCanInstall(ctx)) { needPermission = true; return }
        needPermission = false
        busy = true; error = null; progress = 0f
        scope.launch {
            runCatching {
                val apk = Updater.download(ctx, u) { p -> progress = p }
                progress = null
                Updater.install(ctx, apk)
            }.onFailure { error = it.message ?: "error"; progress = null }
            busy = false
        }
    }

    Scaffold(topBar = {
        TopAppBar(title = { Text(stringResource(R.string.updates)) }, navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, stringResource(R.string.back)) } })
    }) { padding ->
        Column(Modifier.padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Text(stringResource(R.string.update_installed, Updater.current), style = MaterialTheme.typography.titleMedium)
            val u = update
            when {
                busy && progress == null && !checked -> Row(verticalAlignment = Alignment.CenterVertically) { LoadingIndicatorCompat(); Spacer(Modifier.width(12.dp)); Text(stringResource(R.string.update_checking)) }
                u != null -> {
                    Text(stringResource(R.string.update_available_body, u.version), style = MaterialTheme.typography.titleLarge)
                    u.title?.let { Text(it, style = MaterialTheme.typography.titleSmall) }
                    u.notes.forEach { Text("• $it", style = MaterialTheme.typography.bodyMedium) }
                    progress?.let { p -> if (p >= 0) LinearProgressIndicator(progress = { p }, modifier = Modifier.fillMaxWidth()) else LinearProgressIndicator(Modifier.fillMaxWidth()) }
                    Button(onClick = { install(u) }, enabled = !busy, modifier = Modifier.fillMaxWidth().height(52.dp)) { Text(stringResource(if (progress != null) R.string.update_downloading else R.string.update_install)) }
                    if (needPermission) {
                        Text(stringResource(R.string.update_permission), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                        TextButton(onClick = { resume++ }) { Text(stringResource(R.string.retry)) }
                    }
                    Text(stringResource(R.string.update_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                noSource -> Text(stringResource(R.string.update_no_source), color = MaterialTheme.colorScheme.onSurfaceVariant)
                checked -> Text(stringResource(R.string.update_up_to_date), color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            error?.let { Text(stringResource(R.string.update_error, it), color = MaterialTheme.colorScheme.error) }
            OutlinedButton(onClick = { check() }, enabled = !busy) { Text(stringResource(R.string.update_check_now)) }
            HorizontalDivider()
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.update_auto))
                    Text(stringResource(R.string.update_auto_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Switch(checked = auto, onCheckedChange = { auto = it; prefs.edit().putBoolean("autoUpdateCheck", it).apply(); UpdateWorker.schedule(ctx) })
            }
            if (BuildConfig.DEBUG) Text("debug build: updateOverride=" + (prefs.getString("updateOverride", null) ?: "—"), style = MaterialTheme.typography.labelSmall)
        }
    }
}
