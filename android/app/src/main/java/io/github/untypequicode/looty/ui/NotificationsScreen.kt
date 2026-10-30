package io.github.untypequicode.looty.ui

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.outlined.NotificationsActive
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.push.Notifier
import io.github.untypequicode.looty.push.Push
import io.github.untypequicode.looty.push.PushMode
import io.github.untypequicode.looty.push.PushStore
import androidx.lifecycle.compose.LifecycleResumeEffect
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private fun Context.activity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.activity()
    else -> null
}

/**
 * Notifications de cet appareil : push instantané (UnifiedPush), vérification périodique ou rien.
 * Les évènements qui notifient se règlent dans les réglages de l'espace (site).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NotificationsScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }
    // Les réglages changent hors de l'écran (adresse reçue par le service) : relus régulièrement.
    var tick by remember { mutableIntStateOf(0) }
    // Retour des réglages d'Android : état des canaux relu.
    var resumed by remember { mutableIntStateOf(0) }
    LifecycleResumeEffect(Unit) { resumed++; onPauseOrDispose {} }
    LaunchedEffect(Unit) { while (true) { delay(1200); tick++ } }
    val mode = remember(tick) { PushStore.mode(ctx) }
    val registered = remember(tick) { PushStore.subscriptionId(ctx) != null }
    val error = remember(tick) { PushStore.error(ctx) }
    val allowed = remember(tick) { Notifier.permitted(ctx) }
    val distributors = remember { Push.distributors(ctx) }
    val distributorName = remember(tick) { Push.distributorName(ctx) }
    val ws = state.ws

    val askPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { tick++ }
    fun allow() {
        if (Build.VERSION.SDK_INT >= 33) askPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        else ctx.startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, ctx.packageName))
    }
    val failed = stringResource(R.string.push_failed)
    val noDistributor = stringResource(R.string.push_no_distributor)
    val tested = stringResource(R.string.push_test_sent)
    val testNone = stringResource(R.string.push_test_none)
    val testErr = stringResource(R.string.push_test_error)

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.notifications)) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, stringResource(R.string.back)) } },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 24.dp)) {
            if (!allowed) item {
                Card(Modifier.padding(16.dp).fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer)) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(stringResource(R.string.push_blocked), style = MaterialTheme.typography.titleSmall)
                        Text(stringResource(R.string.push_blocked_hint), style = MaterialTheme.typography.bodyMedium)
                        Button(onClick = { allow() }) { Text(stringResource(R.string.push_allow)) }
                    }
                }
            }
            item { SectionTitle(stringResource(R.string.push_how)) }
            // Push instantané
            item {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.push_unified)) },
                    supportingContent = {
                        Text(
                            when {
                                mode == PushMode.UNIFIED_PUSH && registered -> stringResource(R.string.push_unified_on, distributorName ?: "UnifiedPush")
                                mode == PushMode.UNIFIED_PUSH && error == null -> stringResource(R.string.push_registering)
                                distributors.isEmpty() -> stringResource(R.string.push_unified_missing)
                                else -> stringResource(R.string.push_unified_hint)
                            },
                        )
                    },
                    leadingContent = { RadioButton(selected = mode == PushMode.UNIFIED_PUSH, onClick = null) },
                    modifier = Modifier.clickable {
                        val activity = ctx.activity() ?: return@clickable
                        if (distributors.isEmpty()) { scope.launch { snackbar.showSnackbar(noDistributor) }; return@clickable }
                        if (!allowed) allow()
                        Push.enableUnifiedPush(activity) { ok -> tick++; if (!ok) scope.launch { snackbar.showSnackbar(failed) } }
                    },
                )
            }
            if (distributors.isEmpty()) item {
                TextButton(onClick = { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("https://unifiedpush.org/users/distributors/"))) }, modifier = Modifier.padding(start = 56.dp)) {
                    Icon(Icons.AutoMirrored.Outlined.OpenInNew, null, Modifier.size(18.dp)); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.push_get_app))
                }
            } else if (mode == PushMode.UNIFIED_PUSH && distributors.size > 1) item {
                TextButton(onClick = { ctx.activity()?.let { a -> Push.pickDistributor(a) { tick++ } } }, modifier = Modifier.padding(start = 56.dp)) { Text(stringResource(R.string.push_change_app)) }
            }
            // Vérification périodique
            item {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.push_polling)) },
                    supportingContent = { Text(stringResource(R.string.push_polling_hint)) },
                    leadingContent = { RadioButton(selected = mode == PushMode.POLLING, onClick = null) },
                    modifier = Modifier.clickable { if (!allowed) allow(); Push.enablePolling(ctx); tick++ },
                )
            }
            item {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.push_off)) },
                    leadingContent = { RadioButton(selected = mode == PushMode.OFF, onClick = null) },
                    modifier = Modifier.clickable { Push.disable(ctx); tick++ },
                )
            }
            if (error != null && mode != PushMode.OFF) item {
                Text(stringResource(R.string.push_error, error), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(horizontal = 20.dp, vertical = 8.dp))
            }
            if (mode != PushMode.OFF) item {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.push_test)) },
                    leadingContent = { Icon(Icons.Outlined.NotificationsActive, null) },
                    modifier = Modifier.clickable {
                        scope.launch {
                            if (mode == PushMode.UNIFIED_PUSH) {
                                val sent = runCatching { Push.serverTest(ctx) }
                                snackbar.showSnackbar(sent.fold({ if (it > 0) tested else testNone }, { testErr }))
                            } else {
                                Notifier.show(ctx, "Looty", ctx.getString(R.string.push_test_body), null, "test")
                            }
                        }
                    },
                )
            }
            // Canaux Android : son, vibration et importance réglés par type de notification.
            if (Build.VERSION.SDK_INT >= 26) {
                item {
                    HorizontalDivider(Modifier.padding(vertical = 8.dp))
                    ListItem(
                        headlineContent = { Text(stringResource(R.string.ch_settings)) },
                        supportingContent = { Text(stringResource(R.string.ch_settings_hint)) },
                        leadingContent = { Icon(Icons.Outlined.Tune, null) },
                        trailingContent = { Icon(Icons.AutoMirrored.Outlined.OpenInNew, null) },
                        modifier = Modifier.clickable {
                            Notifier.ensureChannels(ctx)
                            ctx.startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, ctx.packageName))
                        },
                    )
                }
                items(Notifier.Kind.entries.size) { i ->
                    val k = Notifier.Kind.entries[i]
                    val on = remember(tick, resumed) { Notifier.channelEnabled(ctx, k) }
                    ListItem(
                        headlineContent = { Text(stringResource(k.title)) },
                        supportingContent = { Text(stringResource(k.hint)) },
                        trailingContent = { if (!on) Text(stringResource(R.string.ch_off), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.labelMedium) },
                        modifier = Modifier.padding(start = 40.dp).clickable {
                            Notifier.ensureChannels(ctx)
                            ctx.startActivity(
                                Intent(Settings.ACTION_CHANNEL_NOTIFICATION_SETTINGS)
                                    .putExtra(Settings.EXTRA_APP_PACKAGE, ctx.packageName)
                                    .putExtra(Settings.EXTRA_CHANNEL_ID, k.channel),
                            )
                        },
                    )
                }
            }
            if (ws != null) item {
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                NotificationPrefsSection(state)
            }
        }
    }
}
