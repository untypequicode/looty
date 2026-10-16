package io.github.untypequicode.looty.ui

import android.widget.Toast
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import kotlinx.coroutines.launch
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject

/** Boîtes mail du compte Looty : liste, ajout, modification. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MailAccountsScreen(state: AppState, onBack: () -> Unit, onEdit: (String) -> Unit) {
    val ctx = LocalContext.current
    LaunchedEffect(Unit) { state.loadMailAccounts() }
    Scaffold(
        topBar = {
            TopAppBar(title = { Text(stringResource(R.string.mail_accounts)) },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } })
        },
        floatingActionButton = { ExtendedFloatingActionButton(onClick = { onEdit("") }, icon = { Icon(Icons.Filled.Add, null) }, text = { Text(stringResource(R.string.add_mail_account)) }) },
    ) { padding ->
        val list = state.mailAccounts.data.orEmpty()
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(bottom = 96.dp)) {
            if (list.isEmpty() && !state.mailAccounts.loading) item { EmptyState(Icons.Outlined.Mail, stringResource(R.string.no_mail_account)) }
            items(list, key = { it.id }) { a ->
                ListItem(
                    headlineContent = { Text(a.label.ifBlank { a.email }, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    supportingContent = {
                        Column {
                            Text(a.email, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            if (!a.can_send) Text(stringResource(R.string.mail_no_sending), style = MaterialTheme.typography.bodySmall)
                            a.last_error?.let { Text(if (it.startsWith("mail_")) errorText(ctx, it) else it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall, maxLines = 2) }
                        }
                    },
                    leadingContent = { Avatar(a.label.ifBlank { a.email }, 40.dp) },
                    trailingContent = { Icon(Icons.Outlined.ChevronRight, null) },
                    modifier = Modifier.clickable { onEdit(a.id) },
                )
            }
        }
    }
}

/**
 * Ajout ou modification d'une boîte mail : réception (IMAP), envoi (SMTP, facultatif), nom,
 * signature, webmail, notifications. Le serveur vérifie la connexion avant d'enregistrer.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MailAccountScreen(state: AppState, id: String, onDone: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val existing = state.mailAccounts.data?.firstOrNull { it.id == id }
    val editing = existing != null
    var label by remember { mutableStateOf(existing?.label.orEmpty()) }
    var email by remember { mutableStateOf(existing?.email.orEmpty()) }
    var host by remember { mutableStateOf(existing?.imap_host.orEmpty()) }
    var security by remember { mutableStateOf(existing?.security ?: "tls") }
    var port by remember { mutableStateOf((existing?.imap_port ?: 993).toString()) }
    var username by remember { mutableStateOf(existing?.username.orEmpty()) }
    var password by remember { mutableStateOf("") }
    var smtpOn by remember { mutableStateOf(existing?.can_send ?: true) }
    var smtpHost by remember { mutableStateOf(existing?.smtp_host.orEmpty()) }
    var smtpSecurity by remember { mutableStateOf(existing?.smtp_security ?: "starttls") }
    var smtpPort by remember { mutableStateOf((existing?.smtp_port ?: 587).toString()) }
    var smtpUser by remember { mutableStateOf(existing?.smtp_username.orEmpty()) }
    var smtpPassword by remember { mutableStateOf("") }
    var displayName by remember { mutableStateOf(existing?.display_name.orEmpty()) }
    var signature by remember { mutableStateOf(existing?.signature.orEmpty()) }
    var webmail by remember { mutableStateOf(existing?.webmail_url.orEmpty()) }
    var notify by remember { mutableStateOf(existing?.notify ?: true) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var confirmDelete by remember { mutableStateOf(false) }
    // Ce qui touche à l'envoi a changé : la configuration SMTP est renvoyée (et vérifiée).
    val smtpKey = listOf(smtpOn, smtpHost, smtpSecurity, smtpPort, smtpUser, smtpPassword).joinToString("|")
    val smtpInitial = remember { smtpKey }

    /** Pré-remplissage depuis l'adresse : imap.<domaine>, identifiant = adresse (comme le site). */
    fun onEmail(v: String) {
        email = v
        if (editing) return
        val domain = v.substringAfter('@', "").trim()
        if (domain.isEmpty() || !domain.contains('.')) return
        if (host.isBlank() || host.startsWith("imap.")) host = "imap.$domain"
        if (username.isBlank() || username.contains('@')) username = v.trim()
        if (label.isBlank() || label.contains('.')) label = domain
        if (smtpHost.isBlank() || smtpHost.startsWith("smtp.")) smtpHost = "smtp.$domain"
    }

    val canSave = !busy && (editing || (label.isNotBlank() && host.isNotBlank() && username.isNotBlank() && password.isNotEmpty()))
    fun smtpBody() = buildJsonObject {
        put("host", smtpHost.trim()); put("port", smtpPort.toIntOrNull() ?: 587); put("security", smtpSecurity)
        smtpUser.trim().takeIf { it.isNotEmpty() }?.let { put("username", it) }
        smtpPassword.takeIf { it.isNotEmpty() }?.let { put("password", it) }
    }
    fun save() {
        busy = true; error = null
        scope.launch {
            val r = if (existing != null) {
                state.updateMailAccount(existing.id, buildJsonObject {
                    put("label", label.trim()); put("webmail_url", webmail.trim()); put("notify", notify)
                    password.takeIf { it.isNotEmpty() }?.let { put("password", it) }
                    put("display_name", displayName); put("signature", signature)
                    if (smtpKey != smtpInitial || password.isNotEmpty()) { if (smtpOn && smtpHost.isNotBlank()) put("smtp", smtpBody()) else put("smtp_remove", true) }
                })
            } else {
                state.createMailAccount(buildJsonObject {
                    put("workspace_id", state.ws.orEmpty()); put("label", label.trim())
                    email.trim().takeIf { it.isNotEmpty() }?.let { put("email", it) }
                    put("imap_host", host.trim()); put("imap_port", port.toIntOrNull() ?: 993); put("security", security)
                    put("username", username.trim()); put("password", password)
                    webmail.trim().takeIf { it.isNotEmpty() }?.let { put("webmail_url", it) }
                    put("notify", notify)
                    displayName.trim().takeIf { it.isNotEmpty() }?.let { put("display_name", it) }
                    signature.trim().takeIf { it.isNotEmpty() }?.let { put("signature", it) }
                    if (smtpOn && smtpHost.isNotBlank()) putJsonObject("smtp") {
                        put("host", smtpHost.trim()); put("port", smtpPort.toIntOrNull() ?: 587); put("security", smtpSecurity)
                        smtpUser.trim().takeIf { it.isNotEmpty() }?.let { put("username", it) }
                        smtpPassword.takeIf { it.isNotEmpty() }?.let { put("password", it) }
                    }
                }).map { }
            }
            busy = false
            r.onSuccess { Toast.makeText(ctx, if (editing) R.string.saved else R.string.mail_account_added, Toast.LENGTH_SHORT).show(); onDone() }
                .onFailure { error = errorText(ctx, it) }
        }
    }

    Scaffold(topBar = {
        TopAppBar(
            title = { Text(stringResource(if (editing) R.string.edit_mail_account else R.string.add_mail_account)) },
            navigationIcon = { IconButton(onClick = onDone) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            actions = {
                if (busy) Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp) }
                else TextButton(enabled = canSave, onClick = { save() }) { Text(stringResource(R.string.save)) }
            },
        )
    }) { padding ->
        Column(Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).imePadding().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            if (busy) Text(stringResource(R.string.checking_connection), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            error?.let {
                Surface(color = MaterialTheme.colorScheme.errorContainer, shape = MaterialTheme.shapes.medium, modifier = Modifier.fillMaxWidth()) {
                    Text(it, color = MaterialTheme.colorScheme.onErrorContainer, modifier = Modifier.padding(12.dp))
                }
            }
            Field(stringResource(R.string.email_address), email, ::onEmail, enabled = !editing, keyboard = KeyboardType.Email)
            Field(stringResource(R.string.account_label), label, { label = it })

            Text(stringResource(R.string.incoming_server), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(top = 8.dp))
            Field(stringResource(R.string.mail_server), host, { host = it }, enabled = !editing, keyboard = KeyboardType.Uri)
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                SecurityChoice(security, listOf("tls", "starttls"), enabled = !editing, modifier = Modifier.weight(1f)) { s ->
                    if (port == "993" || port == "143") port = if (s == "tls") "993" else "143"
                    security = s
                }
                Field(stringResource(R.string.mail_port), port, { port = it.filter(Char::isDigit).take(5) }, enabled = !editing, keyboard = KeyboardType.Number, modifier = Modifier.width(110.dp))
            }
            Field(stringResource(R.string.mail_username), username, { username = it }, enabled = !editing)
            Field(stringResource(R.string.mail_password), password, { password = it }, password = true, supporting = if (editing) stringResource(R.string.password_keep) else null)

            Row(Modifier.fillMaxWidth().padding(top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.sending_enable), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.weight(1f))
                Switch(smtpOn, { smtpOn = it })
            }
            if (smtpOn) {
                Field(stringResource(R.string.mail_server), smtpHost, { smtpHost = it }, keyboard = KeyboardType.Uri)
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                    SecurityChoice(smtpSecurity, listOf("starttls", "tls"), modifier = Modifier.weight(1f)) { s ->
                        if (smtpPort in setOf("465", "587", "25")) smtpPort = if (s == "tls") "465" else "587"
                        smtpSecurity = s
                    }
                    Field(stringResource(R.string.mail_port), smtpPort, { smtpPort = it.filter(Char::isDigit).take(5) }, keyboard = KeyboardType.Number, modifier = Modifier.width(110.dp))
                }
                Field(stringResource(R.string.mail_username), smtpUser, { smtpUser = it }, supporting = stringResource(R.string.smtp_same_login))
                Field(stringResource(R.string.mail_password), smtpPassword, { smtpPassword = it }, password = true, supporting = stringResource(R.string.smtp_same_login))
                Field(stringResource(R.string.display_name), displayName, { displayName = it })
                OutlinedTextField(signature, { signature = it }, label = { Text(stringResource(R.string.mail_signature)) }, minLines = 2, modifier = Modifier.fillMaxWidth())
            }

            Field(stringResource(R.string.webmail_url), webmail, { webmail = it }, keyboard = KeyboardType.Uri)
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.notify_new_mail), modifier = Modifier.weight(1f))
                Switch(notify, { notify = it })
            }
            if (existing != null) {
                HorizontalDivider(Modifier.padding(vertical = 8.dp))
                TextButton(onClick = { confirmDelete = true }) {
                    Icon(Icons.Outlined.Delete, null, tint = MaterialTheme.colorScheme.error); Spacer(Modifier.width(8.dp))
                    Text(stringResource(R.string.remove), color = MaterialTheme.colorScheme.error)
                }
            }
        }
    }
    if (confirmDelete && existing != null) AlertDialog(
        onDismissRequest = { confirmDelete = false },
        title = { Text(stringResource(R.string.delete_mail_account_title, existing.label.ifBlank { existing.email })) },
        text = { Text(stringResource(R.string.delete_mail_account_message)) },
        confirmButton = { TextButton(onClick = { confirmDelete = false; scope.launch { if (state.deleteMailAccount(existing.id)) onDone() } }) { Text(stringResource(R.string.remove), color = MaterialTheme.colorScheme.error) } },
        dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun Field(label: String, value: String, onChange: (String) -> Unit, modifier: Modifier = Modifier.fillMaxWidth(), enabled: Boolean = true,
                  keyboard: KeyboardType = KeyboardType.Text, password: Boolean = false, supporting: String? = null) {
    OutlinedTextField(value, onChange, label = { Text(label) }, singleLine = true, enabled = enabled, modifier = modifier,
        keyboardOptions = KeyboardOptions(keyboardType = if (password) KeyboardType.Password else keyboard),
        visualTransformation = if (password) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        supportingText = supporting?.let { { Text(it) } })
}

/** Sécurité de la connexion : SSL/TLS ou STARTTLS. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SecurityChoice(value: String, options: List<String>, enabled: Boolean = true, modifier: Modifier = Modifier, onChange: (String) -> Unit) {
    SingleChoiceSegmentedButtonRow(modifier) {
        options.forEachIndexed { i, o ->
            SegmentedButton(selected = value == o, onClick = { onChange(o) }, enabled = enabled, shape = SegmentedButtonDefaults.itemShape(i, options.size)) {
                Text(if (o == "tls") "SSL/TLS" else "STARTTLS", maxLines = 1)
            }
        }
    }
}
