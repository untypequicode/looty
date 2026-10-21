package io.github.untypequicode.looty.ui

import android.app.LocaleManager
import android.content.ClipData
import android.content.ClipboardManager
import android.graphics.BitmapFactory
import android.os.Build
import android.os.LocaleList
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.*
import io.github.untypequicode.looty.readSharedFile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.add
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsScaffold(title: String, onBack: () -> Unit, actions: @Composable RowScope.() -> Unit = {}, fab: @Composable () -> Unit = {}, content: @Composable (PaddingValues) -> Unit) {
    Scaffold(
        topBar = { TopAppBar(title = { Text(title) }, navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } }, actions = actions) },
        floatingActionButton = fab,
        content = content,
    )
}

private val isAdminRole: (String) -> Boolean = { it == "owner" || it == "admin" }

/** Réglages : compte, appareil, espace de travail. */
@Composable
fun SettingsScreen(state: AppState, onBack: () -> Unit, onRoute: (String) -> Unit) {
    val ctx = LocalContext.current
    val me = state.me
    val admin = isAdminRole(state.wsRole)
    SettingsScaffold(stringResource(R.string.settings), onBack) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
            item {
                ListItem(
                    headlineContent = { Text(me?.name.orEmpty()) }, supportingContent = { Text(me?.email.orEmpty()) },
                    leadingContent = { UserAvatar(state, me?.name, me?.avatar_url, 48.dp) },
                    modifier = Modifier.clickable { onRoute("settings/profile") },
                )
            }
            item { SectionTitle(stringResource(R.string.settings_account)) }
            @Composable
            fun entry(icon: androidx.compose.ui.graphics.vector.ImageVector, label: Int, hint: Int?, route: String) = ListItem(
                headlineContent = { Text(stringResource(label)) }, supportingContent = hint?.let { { Text(stringResource(it)) } },
                leadingContent = { Icon(icon, null) }, modifier = Modifier.clickable { onRoute(route) },
            )
            item { entry(Icons.Outlined.Person, R.string.settings_profile, null, "settings/profile") }
            item { entry(Icons.Outlined.Palette, R.string.settings_preferences, R.string.settings_preferences_hint, "settings/preferences") }
            item { entry(Icons.Outlined.Notifications, R.string.notifications, R.string.settings_notifications_hint, "notifications") }
            item { entry(Icons.Outlined.EventRepeat, R.string.settings_calendars, R.string.settings_calendars_hint, "settings/calendars") }
            item { entry(Icons.Outlined.Lock, R.string.settings_security, R.string.settings_security_hint, "settings/security") }
            item { SectionTitle(stringResource(R.string.workspace)) }
            item { entry(Icons.Outlined.Workspaces, R.string.settings_workspace, R.string.settings_workspace_hint, "settings/workspace") }
            item { entry(Icons.Outlined.Group, R.string.settings_members, null, "settings/members") }
            if (admin) item {
                ListItem(headlineContent = { Text(stringResource(R.string.settings_roles)) }, supportingContent = { Text(stringResource(R.string.on_website)) },
                    leadingContent = { Icon(Icons.Outlined.AdminPanelSettings, null) }, trailingContent = { Icon(Icons.AutoMirrored.Outlined.OpenInNew, null) },
                    modifier = Modifier.clickable { state.ws?.let { openWeb(ctx, Paths.screen(it, "settings/roles")) } })
            }
            if (me?.is_super_admin == true) item {
                ListItem(headlineContent = { Text(stringResource(R.string.settings_server_admin)) }, supportingContent = { Text(stringResource(R.string.on_website)) },
                    leadingContent = { Icon(Icons.Outlined.Dns, null) }, trailingContent = { Icon(Icons.AutoMirrored.Outlined.OpenInNew, null) },
                    modifier = Modifier.clickable { state.ws?.let { openWeb(ctx, Paths.screen(it, "settings/admin")) } })
            }
        }
    }
}

/** Photo de profil (du serveur), ou les initiales. */
@Composable
fun UserAvatar(state: AppState, name: String?, url: String?, size: Dp) {
    val bitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(null, url) {
        val path = url?.takeIf { it.startsWith("/") } ?: return@produceState
        value = runCatching { state.api.bytes(path).let { b -> BitmapFactory.decodeByteArray(b, 0, b.size)?.asImageBitmap() } }.getOrNull()
    }
    bitmap?.let { Image(it, name, Modifier.size(size).clip(CircleShape), contentScale = ContentScale.Crop) } ?: Avatar(name, size)
}

// ---------------------------------------------------------------------------
// Profil
// ---------------------------------------------------------------------------

@Composable
fun ProfileSettingsScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val me = state.me
    var name by remember { mutableStateOf(me?.name.orEmpty()) }
    var avatar by remember { mutableStateOf(me?.avatar_url) }
    var busy by remember { mutableStateOf(false) }
    val saved = stringResource(R.string.saved)
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        busy = true
        state.launch {
            val f = withContext(Dispatchers.IO) { readSharedFile(ctx, uri) }
            f?.let { state.uploadFile(it).getOrNull() }?.let { up -> if (state.saveProfile(name.ifBlank { me?.name.orEmpty() }, up.url)) avatar = up.url }
            busy = false
        }
    }
    SettingsScaffold(stringResource(R.string.settings_profile), onBack, actions = {
        TextButton(enabled = name.isNotBlank() && name.trim() != me?.name && !busy, onClick = {
            state.launch { if (state.saveProfile(name, avatar)) Toast.makeText(ctx, saved, Toast.LENGTH_SHORT).show() }
        }) { Text(stringResource(R.string.save)) }
    }) { padding ->
        Column(Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Box { UserAvatar(state, name, avatar, 96.dp); if (busy) CircularProgressIndicator(Modifier.size(96.dp)) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(enabled = !busy, onClick = { pick.launch("image/*") }) { Icon(Icons.Outlined.PhotoCamera, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.change_photo)) }
                if (avatar != null) TextButton(enabled = !busy, onClick = { state.launch { if (state.saveProfile(name, null)) avatar = null } }) { Text(stringResource(R.string.remove)) }
            }
            OutlinedTextField(name, { name = it }, singleLine = true, label = { Text(stringResource(R.string.display_name)) }, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(me?.email.orEmpty(), {}, enabled = false, singleLine = true, label = { Text(stringResource(R.string.email_address)) },
                supportingText = { Text(stringResource(R.string.email_cant_change)) }, modifier = Modifier.fillMaxWidth())
        }
    }
}

// ---------------------------------------------------------------------------
// Préférences
// ---------------------------------------------------------------------------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PreferencesScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val theme = AppTheme.load(ctx)
    val locales = if (Build.VERSION.SDK_INT >= 33) ctx.getSystemService(LocaleManager::class.java) else null
    var lang by remember { mutableStateOf(locales?.applicationLocales?.takeIf { !it.isEmpty }?.get(0)?.language ?: "") }
    SettingsScaffold(stringResource(R.string.settings_preferences), onBack) { padding ->
        Column(Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.theme), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
            val themes = listOf("system" to R.string.theme_system, "light" to R.string.theme_light, "dark" to R.string.theme_dark)
            SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                themes.forEachIndexed { i, (m, label) ->
                    SegmentedButton(selected = theme == m, onClick = { AppTheme.set(ctx, m); state.launch { state.savePreference("theme", m) } },
                        shape = SegmentedButtonDefaults.itemShape(i, themes.size)) { Text(stringResource(label), maxLines = 1) }
                }
            }
            Text(stringResource(R.string.theme_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            HorizontalDivider(Modifier.padding(vertical = 8.dp))
            Text(stringResource(R.string.language), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
            if (locales != null) {
                val langs = listOf("" to R.string.language_system, "en" to R.string.language_en, "fr" to R.string.language_fr)
                langs.forEach { (code, label) ->
                    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).clickable {
                        lang = code
                        // Langue de l'application (Android 13+) ; celle des e-mails suit sur le serveur.
                        locales.applicationLocales = if (code.isEmpty()) LocaleList.getEmptyLocaleList() else LocaleList.forLanguageTags(code)
                        val serverLocale = code.ifEmpty { java.util.Locale.getDefault().language.takeIf { it == "fr" } ?: "en" }
                        state.launch { state.savePreference("locale", serverLocale) }
                    }, verticalAlignment = Alignment.CenterVertically) { RadioButton(lang == code, null); Spacer(Modifier.width(8.dp)); Text(stringResource(label)) }
                }
            } else Text(stringResource(R.string.language_follows_phone), style = MaterialTheme.typography.bodyMedium)
            Text(stringResource(R.string.language_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// ---------------------------------------------------------------------------
// Notifications : ce qui est envoyé, par e-mail et en push (section de l'écran Notifications)
// ---------------------------------------------------------------------------

private val EVENT_LABELS = mapOf(
    "task_assigned" to R.string.ev_task_assigned, "task_mention" to R.string.ev_task_mention, "doc_mention" to R.string.ev_doc_mention,
    "task_comment" to R.string.ev_task_comment, "task_due" to R.string.ev_task_due, "task_completed" to R.string.ev_task_completed,
    "mail_received" to R.string.ev_mail_received, "moodle_received" to R.string.ev_moodle_received,
)

@Composable
fun NotificationPrefsSection(state: AppState) {
    val ctx = LocalContext.current
    var prefs by remember { mutableStateOf<NotifPrefs?>(null) }
    var devices by remember { mutableStateOf<List<PushDevice>?>(null) }
    LaunchedEffect(Unit) { prefs = state.notifPrefs(); devices = state.pushDevices() }
    fun save(body: kotlinx.serialization.json.JsonObject) = state.launch { state.saveNotifPrefs(body)?.let { prefs = it } }
    val p = prefs ?: return
    Column {
        SectionTitle(stringResource(R.string.notif_what))
        Text(stringResource(R.string.notif_what_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp))
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 8.dp)) {
            Spacer(Modifier.weight(1f))
            if (p.email_available) Text(stringResource(R.string.channel_email), style = MaterialTheme.typography.labelMedium, modifier = Modifier.width(64.dp))
            Text(stringResource(R.string.channel_push), style = MaterialTheme.typography.labelMedium, modifier = Modifier.width(56.dp))
        }
        p.events.forEach { e ->
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(EVENT_LABELS[e.event]?.let { stringResource(it) } ?: e.event, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium)
                fun send(next: EventPref) = save(buildJsonObject { putJsonArray("events") { add(buildJsonObject { put("event", next.event); put("email", next.email); put("push", next.push) }) } })
                if (p.email_available) Box(Modifier.width(64.dp)) { Checkbox(e.email, { send(e.copy(email = it)) }, enabled = p.email_enabled) }
                Box(Modifier.width(56.dp)) { Checkbox(e.push, { send(e.copy(push = it)) }) }
            }
        }
        if (p.email_available) ListItem(
            headlineContent = { Text(stringResource(R.string.notif_email)) }, supportingContent = { Text(p.email) },
            trailingContent = { Switch(p.email_enabled, { save(buildJsonObject { put("email_enabled", it) }) }) },
        )
        devices?.takeIf { it.isNotEmpty() }?.let { list ->
            SectionTitle(stringResource(R.string.notif_devices))
            list.forEach { d ->
                ListItem(
                    headlineContent = { Text(d.device ?: d.endpoint_host, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = { Text(listOfNotNull(d.endpoint_host.takeIf { d.device != null }, d.last_used_at?.let { relativeTime(it, ctx) }).joinToString(" · ")) },
                    leadingContent = { Icon(Icons.Outlined.Devices, null) },
                    trailingContent = { IconButton(onClick = { state.launch { if (state.removePushDevice(d.id)) devices = state.pushDevices() } }) { Icon(Icons.Outlined.Delete, stringResource(R.string.remove)) } },
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Sécurité : jetons d'API, comptes de connexion liés
// ---------------------------------------------------------------------------

@Composable
fun SecurityScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    var tokens by remember { mutableStateOf<List<ApiToken>?>(null) }
    var ids by remember { mutableStateOf<List<Identity>?>(null) }
    var newToken by remember { mutableStateOf(false) }
    var created by remember { mutableStateOf<String?>(null) }
    var revoke by remember { mutableStateOf<ApiToken?>(null) }
    var unlink by remember { mutableStateOf<Identity?>(null) }
    val copied = stringResource(R.string.link_copied)
    fun reload() = state.launch { tokens = state.apiTokens(); ids = state.identities() }
    LaunchedEffect(Unit) { reload() }
    SettingsScaffold(stringResource(R.string.settings_security), onBack) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
            item { SectionTitle(stringResource(R.string.linked_accounts)) }
            item { Text(stringResource(R.string.linked_accounts_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp)) }
            if (ids?.isEmpty() == true) item { Text(stringResource(R.string.none), modifier = Modifier.padding(16.dp)) }
            items(ids.orEmpty(), key = { "i" + it.provider }) { i ->
                ListItem(headlineContent = { Text(i.provider_name ?: stringResource(R.string.unknown_provider)) }, supportingContent = { Text(mailFullDate(i.created_at)) },
                    leadingContent = { Icon(Icons.Outlined.Key, null) },
                    trailingContent = { if ((ids?.size ?: 0) > 1) TextButton(onClick = { unlink = i }) { Text(stringResource(R.string.unlink)) } })
            }
            item {
                SectionTitle(stringResource(R.string.api_tokens), action = { TextButton(onClick = { newToken = true }) { Icon(Icons.Outlined.Add, null, Modifier.size(18.dp)); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.new_token)) } })
            }
            item { Text(stringResource(R.string.api_tokens_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp)) }
            if (tokens?.none { it.scope != "ics" } == true) item { Text(stringResource(R.string.none), modifier = Modifier.padding(16.dp)) }
            items(tokens.orEmpty().filter { it.scope != "ics" }, key = { "t" + it.id }) { t ->
                ListItem(headlineContent = { Text(t.name) },
                    supportingContent = { Text(t.last_used_at?.let { stringResource(R.string.last_used, relativeTime(it, ctx)) } ?: stringResource(R.string.never_used)) },
                    leadingContent = { Icon(if (t.scope == "caldav") Icons.Outlined.EventRepeat else Icons.Outlined.Token, null) },
                    trailingContent = { TextButton(onClick = { revoke = t }) { Text(stringResource(R.string.revoke), color = MaterialTheme.colorScheme.error) } })
            }
        }
    }
    if (newToken) NameDialog(stringResource(R.string.new_token), stringResource(R.string.token_name), "", onDismiss = { newToken = false }) { n ->
        newToken = false; state.launch { state.createApiToken(n)?.let { created = it; reload() } }
    }
    created?.let { tk ->
        AlertDialog(
            onDismissRequest = { created = null },
            title = { Text(stringResource(R.string.new_token)) },
            text = { Column(verticalArrangement = Arrangement.spacedBy(8.dp)) { Text(stringResource(R.string.token_copy_now)); Text(tk, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall) } },
            confirmButton = { TextButton(onClick = { ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("token", tk)); Toast.makeText(ctx, copied, Toast.LENGTH_SHORT).show() }) { Text(stringResource(R.string.copy)) } },
            dismissButton = { TextButton(onClick = { created = null }) { Text(stringResource(R.string.ok)) } },
        )
    }
    revoke?.let { t ->
        AlertDialog(onDismissRequest = { revoke = null }, title = { Text(stringResource(R.string.revoke_token_title, t.name)) }, text = { Text(stringResource(R.string.revoke_token_message)) },
            confirmButton = { TextButton(onClick = { revoke = null; state.launch { if (state.revokeApiToken(t.id)) reload() } }) { Text(stringResource(R.string.revoke), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { revoke = null }) { Text(stringResource(R.string.cancel)) } })
    }
    unlink?.let { i ->
        AlertDialog(onDismissRequest = { unlink = null }, title = { Text(stringResource(R.string.unlink_title, i.provider_name ?: stringResource(R.string.unknown_provider))) },
            confirmButton = { TextButton(onClick = { unlink = null; state.launch { if (state.unlinkIdentity(i.provider)) reload() } }) { Text(stringResource(R.string.unlink), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { unlink = null }) { Text(stringResource(R.string.cancel)) } })
    }
}

// ---------------------------------------------------------------------------
// Espace de travail : nom, validations collectives, corbeille, drive, Moodle, quitter, supprimer
// ---------------------------------------------------------------------------

@Composable
fun WorkspaceSettingsScreen(state: AppState, onBack: () -> Unit, onLeft: () -> Unit) {
    val ctx = LocalContext.current
    val role = state.wsRole
    val admin = isAdminRole(role)
    val session = state.session as? io.github.untypequicode.looty.Session.Ready
    val wsName = session?.workspaces?.firstOrNull { it.id == state.ws }?.name.orEmpty()
    var name by remember(wsName) { mutableStateOf(wsName) }
    var retention by remember { mutableStateOf<Int?>(null) }
    var drive by remember { mutableStateOf<WorkspaceDrive?>(null) }
    var driveDays by remember { mutableStateOf("") }
    var driveQuota by remember { mutableStateOf("") }
    var moodleUrl by remember { mutableStateOf("") }
    var moodleDefault by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf<String?>(null) }
    val saved = stringResource(R.string.saved)
    fun ok(b: Boolean) { if (b) Toast.makeText(ctx, saved, Toast.LENGTH_SHORT).show() }
    LaunchedEffect(admin) {
        if (admin) {
            retention = state.trashRetention()
            drive = state.driveSettings()?.also { driveDays = it.max_days.toString(); driveQuota = (it.quota_bytes / 1048576).toString() }
            state.moodleOverview()?.let { moodleUrl = it.url.orEmpty(); moodleDefault = it.default_url }
        }
    }
    SettingsScaffold(stringResource(R.string.settings_workspace), onBack) { padding ->
        Column(Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedTextField(name, { name = it }, singleLine = true, enabled = admin, label = { Text(stringResource(R.string.workspace_name)) }, modifier = Modifier.fillMaxWidth(),
                trailingIcon = { if (admin && name.isNotBlank() && name.trim() != wsName) IconButton(onClick = { state.launch { ok(state.renameWorkspace(name)) } }) { Icon(Icons.Outlined.Check, stringResource(R.string.save)) } })
            Text(stringResource(R.string.your_role, state.access?.role_name ?: role), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            ListItem(colors = sheetItemColors(), headlineContent = { Text(stringResource(R.string.group_tasks)) }, supportingContent = { Text(stringResource(R.string.group_tasks_hint)) },
                trailingContent = { Switch(state.access?.group_tasks ?: true, { on -> state.launch { state.setGroupTasks(on) } }) })
            if (admin) {
                HorizontalDivider()
                Text(stringResource(R.string.trash), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                var menu by remember { mutableStateOf(false) }
                Box {
                    OutlinedButton(onClick = { menu = true }) { Text(retention?.let { if (it == 0) stringResource(R.string.keep_forever) else androidx.compose.ui.res.pluralStringResource(R.plurals.n_days, it, it) } ?: "…"); Icon(Icons.Filled.ArrowDropDown, null) }
                    DropdownMenu(menu, { menu = false }) {
                        listOf(7, 14, 30, 60, 90, 180, 365, 0).forEach { d ->
                            DropdownMenuItem(text = { Text(if (d == 0) stringResource(R.string.keep_forever) else androidx.compose.ui.res.pluralStringResource(R.plurals.n_days, d, d)) },
                                leadingIcon = { RadioButton(retention == d, null) }, onClick = { menu = false; state.launch { if (state.setTrashRetention(d)) { retention = d; ok(true) } } })
                        }
                    }
                }
                Text(stringResource(R.string.trash_retention_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (drive != null) {
                    HorizontalDivider()
                    Text(stringResource(R.string.drive), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        OutlinedTextField(driveDays, { driveDays = it.filter(Char::isDigit).take(3) }, singleLine = true, label = { Text(stringResource(R.string.drive_max_days)) },
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), modifier = Modifier.weight(1f))
                        OutlinedTextField(driveQuota, { driveQuota = it.filter(Char::isDigit).take(7) }, singleLine = true, label = { Text(stringResource(R.string.drive_quota_mb)) },
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), modifier = Modifier.weight(1f))
                    }
                    OutlinedButton(enabled = driveDays.isNotEmpty() && driveQuota.isNotEmpty(), onClick = { state.launch { ok(state.setDriveSettings(driveDays.toInt(), driveQuota.toInt())) } }) { Text(stringResource(R.string.save)) }
                }
                HorizontalDivider()
                Text("Moodle", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                OutlinedTextField(moodleUrl, { moodleUrl = it }, singleLine = true, label = { Text(stringResource(R.string.moodle_address)) }, placeholder = { Text(moodleDefault) },
                    supportingText = { Text(stringResource(R.string.moodle_address_hint)) }, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri), modifier = Modifier.fillMaxWidth())
                OutlinedButton(onClick = { state.launch { ok(state.setMoodleUrl(moodleUrl.trim().ifEmpty { null })) } }) { Text(stringResource(R.string.save)) }
            }
            HorizontalDivider()
            OutlinedButton(onClick = { confirm = "leave" }, colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error)) {
                Icon(Icons.Outlined.Logout, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.leave_workspace))
            }
            if (role == "owner") OutlinedButton(onClick = { confirm = "delete" }, colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error)) {
                Icon(Icons.Outlined.DeleteForever, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.delete_workspace))
            }
        }
    }
    confirm?.let { what ->
        AlertDialog(
            onDismissRequest = { confirm = null },
            title = { Text(stringResource(if (what == "leave") R.string.leave_workspace_title else R.string.delete_workspace_title, wsName)) },
            text = { Text(stringResource(if (what == "leave") R.string.leave_workspace_message else R.string.delete_workspace_message)) },
            confirmButton = {
                TextButton(onClick = {
                    confirm = null
                    state.launch { if (if (what == "leave") state.leaveWorkspace() else state.deleteWorkspace()) { state.start(); onLeft() } }
                }) { Text(stringResource(if (what == "leave") R.string.leave else R.string.delete), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirm = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

// ---------------------------------------------------------------------------
// Membres : rôles, ajout (compte existant ou invitation par e-mail), invitations en attente
// ---------------------------------------------------------------------------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MembersScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val myRole = state.wsRole
    val admin = isAdminRole(myRole)
    val me = state.me?.id
    var roles by remember { mutableStateOf<List<WsRole>>(emptyList()) }
    var invites by remember { mutableStateOf<List<Invite>>(emptyList()) }
    var adding by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<Member?>(null) }
    LaunchedEffect(Unit) { state.loadMembers(); roles = state.roles().orEmpty(); if (admin) invites = state.invites().orEmpty() }
    // Rôles qu'on peut donner (comme sur le site) : jamais « propriétaire », et les rôles d'admin seulement par le propriétaire.
    val assignable = roles.filter { it.preset != "owner" && (myRole == "owner" || !it.permissions.admin) }
    fun canChange(m: Member) = admin && m.user_id != me && (myRole == "owner" || (m.role != "owner" && m.role != "admin"))
    SettingsScaffold(stringResource(R.string.settings_members), onBack, fab = {
        if (admin || state.access?.permissions?.invite == true) ExtendedFloatingActionButton(onClick = { adding = true }, icon = { Icon(Icons.Outlined.PersonAdd, null) }, text = { Text(stringResource(R.string.add_people)) })
    }) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
            items(state.members.values.sortedWith(compareBy({ listOf("owner", "admin", "member", "viewer").indexOf(it.role).let { i -> if (i < 0) 9 else i } }, { it.name })), key = { it.user_id }) { m ->
                var menu by remember { mutableStateOf(false) }
                ListItem(
                    headlineContent = { Text(m.name + if (m.user_id == me) " · " + stringResource(R.string.you) else "", maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = { Text(m.email, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    leadingContent = { UserAvatar(state, m.name, m.avatar_url, 40.dp) },
                    trailingContent = {
                        Box {
                            val label = m.role_name ?: m.role
                            if (canChange(m)) {
                                TextButton(onClick = { menu = true }) { Text(label, maxLines = 1); Icon(Icons.Filled.ArrowDropDown, null) }
                                DropdownMenu(menu, { menu = false }) {
                                    assignable.forEach { r ->
                                        DropdownMenuItem(text = { Text(r.name) }, leadingIcon = { RadioButton(r.id == m.role_id || (m.role_id == null && r.preset == m.role), null) },
                                            onClick = { menu = false; state.launch { state.setMemberRole(m.user_id, r.id) } })
                                    }
                                    HorizontalDivider()
                                    DropdownMenuItem(text = { Text(stringResource(R.string.remove_from_workspace), color = MaterialTheme.colorScheme.error) }, onClick = { menu = false; removing = m })
                                }
                            } else Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    },
                )
            }
            if (invites.isNotEmpty()) {
                item { SectionTitle(stringResource(R.string.pending_invites)) }
                items(invites, key = { it.id }) { i ->
                    ListItem(
                        headlineContent = { Text(i.role) },
                        supportingContent = { Text(listOfNotNull(i.created_by_name, i.expires_at?.let { stringResource(R.string.expires_on, mailFullDate(it)) }).joinToString(" · ")) },
                        leadingContent = { Icon(Icons.Outlined.MailOutline, null) },
                        trailingContent = { TextButton(onClick = { state.launch { if (state.revokeInvite(i.id)) invites = state.invites().orEmpty() } }) { Text(stringResource(R.string.revoke), color = MaterialTheme.colorScheme.error) } },
                    )
                }
            }
        }
    }
    // Liste des invitations rechargée à la fermeture (une invitation vient peut-être d'être créée).
    if (adding) AddPeopleSheet(state, assignable, onDismiss = { adding = false; state.launch { if (admin) invites = state.invites().orEmpty() } }) { state.launch { if (admin) invites = state.invites().orEmpty() } }
    removing?.let { m ->
        AlertDialog(onDismissRequest = { removing = null }, title = { Text(stringResource(R.string.remove_member_title, m.name)) }, text = { Text(stringResource(R.string.remove_member_message)) },
            confirmButton = { TextButton(onClick = { removing = null; state.launch { state.removeMember(m.user_id) } }) { Text(stringResource(R.string.remove), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { removing = null }) { Text(stringResource(R.string.cancel)) } })
    }
}

/** Ajouter quelqu'un : un compte Looty existant, ou une invitation envoyée à une adresse e-mail. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddPeopleSheet(state: AppState, roles: List<WsRole>, onDismiss: () -> Unit, onDone: () -> Unit) {
    val ctx = LocalContext.current
    var query by remember { mutableStateOf("") }
    var hits by remember { mutableStateOf<List<UserHit>>(emptyList()) }
    var roleId by remember { mutableStateOf(roles.firstOrNull { it.preset == "member" }?.id ?: roles.firstOrNull()?.id) }
    var result by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    LaunchedEffect(query) { kotlinx.coroutines.delay(250); hits = if (query.trim().length >= 2) state.searchUsers(query.trim()) else emptyList() }
    val isEmail = Regex("^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$").matches(query.trim())
    fun add(userId: String?, email: String?) {
        busy = true; result = null
        state.launch {
            state.addMember(userId, email, roleId)
                .onSuccess { r ->
                    result = when {
                        r.added != null -> ctx.getString(R.string.member_added)
                        r.email_sent -> ctx.getString(R.string.invite_sent, email.orEmpty())
                        r.link != null -> { ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("invite", r.link)); ctx.getString(R.string.invite_link_copied) }
                        else -> ctx.getString(R.string.saved)
                    }
                    query = ""; onDone()
                }.onFailure { result = errorText(ctx, it) }
            busy = false
        }
    }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        NoNavScrim()
        Column(Modifier.verticalScroll(rememberScrollState()).navigationBarsPadding().imePadding().padding(bottom = 16.dp)) {
            Text(stringResource(R.string.add_people), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp))
            Text(stringResource(R.string.add_people_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 24.dp))
            if (roles.isNotEmpty()) {
                var menu by remember { mutableStateOf(false) }
                Box(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                    OutlinedButton(onClick = { menu = true }) { Text(stringResource(R.string.role_label, roles.firstOrNull { it.id == roleId }?.name.orEmpty())); Icon(Icons.Filled.ArrowDropDown, null) }
                    DropdownMenu(menu, { menu = false }) { roles.forEach { r -> DropdownMenuItem(text = { Text(r.name) }, leadingIcon = { RadioButton(r.id == roleId, null) }, onClick = { menu = false; roleId = r.id }) } }
                }
            }
            OutlinedTextField(query, { query = it }, singleLine = true, placeholder = { Text(stringResource(R.string.name_or_email)) }, leadingIcon = { Icon(Icons.Outlined.Search, null) },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email), modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp))
            if (busy) LinearProgressIndicator(Modifier.fillMaxWidth().padding(16.dp))
            result?.let { Text(it, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(16.dp)) }
            hits.forEach { u ->
                ListItem(colors = sheetItemColors(), headlineContent = { Text(u.name) }, supportingContent = { Text(u.email) }, leadingContent = { Avatar(u.name, 32.dp) },
                    trailingContent = { TextButton(enabled = !busy, onClick = { add(u.id, null) }) { Text(stringResource(R.string.add)) } })
            }
            if (isEmail && hits.none { it.email.equals(query.trim(), true) }) ListItem(colors = sheetItemColors(),
                headlineContent = { Text(stringResource(R.string.invite_by_email)) }, supportingContent = { Text(query.trim()) }, leadingContent = { Icon(Icons.Outlined.ForwardToInbox, null) },
                trailingContent = { TextButton(enabled = !busy, onClick = { add(null, query.trim()) }) { Text(stringResource(R.string.invite)) } })
        }
    }
}

// ---------------------------------------------------------------------------
// Calendriers : synchronisation des tâches avec un calendrier CalDAV (Nextcloud, iCloud…)
// ---------------------------------------------------------------------------

@Composable
fun CalendarSyncScreen(state: AppState, onBack: () -> Unit) {
    val ctx = LocalContext.current
    var list by remember { mutableStateOf<List<CalDavAccount>?>(null) }
    var adding by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf<CalDavAccount?>(null) }
    var syncing by remember { mutableStateOf<String?>(null) }
    val res = ctx.resources
    fun reload() = state.launch { list = state.calDavAccounts() ?: list }
    LaunchedEffect(Unit) { reload() }
    SettingsScaffold(stringResource(R.string.settings_calendars), onBack, fab = {
        ExtendedFloatingActionButton(onClick = { adding = true }, icon = { Icon(Icons.Outlined.Add, null) }, text = { Text(stringResource(R.string.connect_calendar)) })
    }) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
            item { PhoneCalendarSection(state) }
            item { Text(stringResource(R.string.caldav_hint), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(16.dp)) }
            val l = list
            if (l == null) item { Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp)) } }
            else if (l.isEmpty()) item { EmptyState(Icons.Outlined.EventRepeat, stringResource(R.string.no_caldav)) }
            items(l.orEmpty(), key = { it.id }) { a ->
                var menu by remember { mutableStateOf(false) }
                ListItem(
                    headlineContent = { Text(a.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = {
                        Column {
                            Text(listOfNotNull(a.calendar_name, androidx.compose.ui.res.pluralStringResource(R.plurals.n_tasks, a.task_count.toInt(), a.task_count.toInt())).joinToString(" · "))
                            Text(a.last_sync_at?.let { stringResource(R.string.last_sync, relativeTime(it, ctx)) } ?: stringResource(R.string.never_synced), style = MaterialTheme.typography.labelSmall)
                            a.last_error?.let { Text(if (it.startsWith("caldav_")) errorText(ctx, it) else it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.labelSmall, maxLines = 2) }
                        }
                    },
                    leadingContent = { Box(Modifier.size(14.dp).clip(CircleShape).background(a.calendar_color?.let { labelColor(it) } ?: MaterialTheme.colorScheme.primary)) },
                    trailingContent = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Switch(a.enabled, { on -> state.launch { if (state.updateCalDav(a.id, buildJsonObject { put("enabled", on) })) reload() } })
                            Box {
                                IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                                DropdownMenu(menu, { menu = false }) {
                                    DropdownMenuItem(text = { Text(stringResource(R.string.sync_now)) }, leadingIcon = { Icon(Icons.Outlined.Sync, null) }, enabled = syncing == null, onClick = {
                                        menu = false; syncing = a.id
                                        state.launch {
                                            state.syncCalDav(a.id)?.let { r -> Toast.makeText(ctx, res.getString(R.string.sync_report, r.pushed, r.pulled), Toast.LENGTH_LONG).show() }
                                            syncing = null; reload()
                                        }
                                    })
                                    DropdownMenuItem(text = { Text(stringResource(R.string.include_completed)) }, leadingIcon = { Checkbox(a.include_completed, null) },
                                        onClick = { menu = false; state.launch { if (state.updateCalDav(a.id, buildJsonObject { put("include_completed", !a.include_completed) })) reload() } })
                                    DropdownMenuItem(text = { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) }, leadingIcon = { Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error) }, onClick = { menu = false; deleting = a })
                                }
                            }
                        }
                    },
                )
                if (syncing == a.id) LinearProgressIndicator(Modifier.fillMaxWidth().padding(horizontal = 16.dp))
            }
        }
    }
    if (adding) AddCalDavDialog(state, onDismiss = { adding = false }) { adding = false; reload() }
    deleting?.let { a ->
        AlertDialog(onDismissRequest = { deleting = null }, title = { Text(stringResource(R.string.delete_caldav_title, a.name)) }, text = { Text(stringResource(R.string.delete_caldav_message)) },
            confirmButton = { TextButton(onClick = { deleting = null; state.launch { if (state.deleteCalDav(a.id)) reload() } }) { Text(stringResource(R.string.delete), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = { deleting = null }) { Text(stringResource(R.string.cancel)) } })
    }
}

/** Connexion d'un calendrier : serveur et identifiants, puis choix du calendrier et des tableaux. */
@Composable
private fun AddCalDavDialog(state: AppState, onDismiss: () -> Unit, onAdded: () -> Unit) {
    val ctx = LocalContext.current
    var server by remember { mutableStateOf("") }
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var found by remember { mutableStateOf<List<DiscoveredCalendar>?>(null) }
    var chosen by remember { mutableStateOf<DiscoveredCalendar?>(null) }
    var boards by remember { mutableStateOf<Set<String>?>(null) } // null = tous les tableaux
    var error by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.connect_calendar)) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                if (busy) LinearProgressIndicator(Modifier.fillMaxWidth())
                if (found == null) {
                    Text(stringResource(R.string.caldav_server_hint), style = MaterialTheme.typography.bodySmall)
                    OutlinedTextField(server, { server = it }, singleLine = true, label = { Text(stringResource(R.string.mail_server)) }, placeholder = { Text("https://cloud.example.com/remote.php/dav") },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri), modifier = Modifier.fillMaxWidth())
                    OutlinedTextField(user, { user = it }, singleLine = true, label = { Text(stringResource(R.string.mail_username)) }, modifier = Modifier.fillMaxWidth())
                    OutlinedTextField(password, { password = it }, singleLine = true, label = { Text(stringResource(R.string.app_password)) }, visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password), modifier = Modifier.fillMaxWidth())
                } else {
                    Text(stringResource(R.string.pick_calendar), style = MaterialTheme.typography.titleSmall)
                    found!!.filter { it.supports_todo }.ifEmpty { found!! }.forEach { c ->
                        Row(Modifier.fillMaxWidth().clickable { chosen = c }, verticalAlignment = Alignment.CenterVertically) {
                            RadioButton(chosen == c, { chosen = c })
                            Box(Modifier.size(10.dp).clip(CircleShape).background(c.color?.let { labelColor(it) } ?: Color.Gray)); Spacer(Modifier.width(8.dp)); Text(c.name)
                        }
                    }
                    Text(stringResource(R.string.boards), style = MaterialTheme.typography.titleSmall)
                    Row(Modifier.fillMaxWidth().clickable { boards = if (boards == null) emptySet() else null }, verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(boards == null, { boards = if (it) null else emptySet() }); Text(stringResource(R.string.all_boards))
                    }
                    if (boards != null) state.boards.forEach { b ->
                        Row(Modifier.fillMaxWidth().clickable { boards = if (b.id in boards!!) boards!! - b.id else boards!! + b.id }, verticalAlignment = Alignment.CenterVertically) {
                            Checkbox(b.id in boards!!, null); Text(b.name)
                        }
                    }
                }
            }
        },
        confirmButton = {
            if (found == null) TextButton(enabled = !busy && server.isNotBlank() && user.isNotBlank() && password.isNotEmpty(), onClick = {
                busy = true; error = null
                state.launch {
                    state.discoverCalDav(server, user, password).onSuccess { l -> if (l.isEmpty()) error = ctx.getString(R.string.no_calendar_found) else { found = l; chosen = l.firstOrNull { it.supports_todo } ?: l.first() } }
                        .onFailure { error = errorText(ctx, it) }
                    busy = false
                }
            }) { Text(stringResource(R.string.next)) }
            else TextButton(enabled = !busy && chosen != null, onClick = {
                val c = chosen!!
                busy = true; error = null
                state.launch {
                    state.createCalDav(buildJsonObject {
                        put("name", c.name); put("server_url", server.trim()); put("calendar_url", c.url); put("calendar_name", c.name); c.color?.let { put("calendar_color", it) }
                        put("username", user.trim()); put("password", password)
                        boards?.let { s -> putJsonArray("board_ids") { s.forEach { add(it) } } } ?: put("board_ids", JsonNull)
                    }).onSuccess { onAdded() }.onFailure { error = errorText(ctx, it) }
                    busy = false
                }
            }) { Text(stringResource(R.string.connect)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

// ---------------------------------------------------------------------------
// Agenda du téléphone : les calendriers de Looty dans l'application Agenda, via DAVx⁵
// ---------------------------------------------------------------------------

private const val DAVX5 = "at.bitfire.davdroid"

private fun davx5Installed(ctx: android.content.Context) = runCatching { ctx.packageManager.getPackageInfo(DAVX5, 0) }.isSuccess

private data class DavCredentials(val server: String, val username: String, val password: String)

/**
 * Connexion à l'agenda du téléphone : un mot de passe d'application CalDAV est créé, puis DAVx⁵ s'ouvre
 * sur son écran de connexion déjà rempli. Sans DAVx⁵ : proposition de l'installer, ou des identifiants à copier.
 */
@Composable
private fun PhoneCalendarSection(state: AppState) {
    val ctx = LocalContext.current
    var missing by remember { mutableStateOf(false) }
    var creds by remember { mutableStateOf<DavCredentials?>(null) }
    var busy by remember { mutableStateOf(false) }
    // Crée le mot de passe, puis ouvre DAVx⁵ ; en cas d'échec d'ouverture, les identifiants sont montrés.
    fun connect(openDavx5: Boolean) {
        busy = true
        state.launch {
            val password = state.createCalDavPassword("Android · ${Build.MODEL}")
            busy = false
            if (password == null) return@launch
            val username = state.me?.email ?: return@launch
            val server = state.api.server.trimEnd('/') + "/dav/"
            val intent = android.content.Intent().setClassName(DAVX5, "at.bitfire.davdroid.ui.setup.LoginActivity")
                .putExtra("url", server).putExtra("username", username).putExtra("password", password)
            if (!openDavx5 || runCatching { ctx.startActivity(intent) }.isFailure) creds = DavCredentials(server, username, password)
        }
    }
    ListItem(
        headlineContent = { Text(stringResource(R.string.phone_calendar_title)) },
        supportingContent = { Text(stringResource(R.string.phone_calendar_hint)) },
        leadingContent = { Icon(Icons.Outlined.PhoneAndroid, null) },
        trailingContent = {
            if (busy) CircularProgressIndicator(Modifier.size(24.dp))
            else OutlinedButton(onClick = { if (davx5Installed(ctx)) connect(true) else missing = true }) { Text(stringResource(R.string.phone_calendar_connect)) }
        },
    )
    HorizontalDivider()
    if (missing) {
        AlertDialog(
            onDismissRequest = { missing = false },
            title = { Text(stringResource(R.string.davx5_missing_title)) },
            text = { Text(stringResource(R.string.davx5_missing_message)) },
            confirmButton = {
                TextButton(onClick = {
                    missing = false
                    val market = android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse("market://details?id=$DAVX5"))
                    val web = android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse("https://f-droid.org/packages/$DAVX5/"))
                    if (runCatching { ctx.startActivity(market) }.isFailure) runCatching { ctx.startActivity(web) }
                }) { Text(stringResource(R.string.davx5_install)) }
            },
            dismissButton = { TextButton(onClick = { missing = false; connect(false) }) { Text(stringResource(R.string.enter_manually)) } },
        )
    }
    creds?.let { c ->
        val copied = stringResource(R.string.link_copied)
        fun copy(label: String, text: String) { ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(label, text)); Toast.makeText(ctx, copied, Toast.LENGTH_SHORT).show() }
        AlertDialog(
            onDismissRequest = { creds = null },
            title = { Text(stringResource(R.string.phone_calendar_creds_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(stringResource(R.string.phone_calendar_creds_message))
                    Text(c.server, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall, modifier = Modifier.clickable { copy("server", c.server) })
                    Text(c.username, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall, modifier = Modifier.clickable { copy("username", c.username) })
                    Text(c.password, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                }
            },
            confirmButton = { TextButton(onClick = { copy("password", c.password) }) { Text(stringResource(R.string.copy)) } },
            dismissButton = { TextButton(onClick = { creds = null }) { Text(stringResource(R.string.ok)) } },
        )
    }
}
