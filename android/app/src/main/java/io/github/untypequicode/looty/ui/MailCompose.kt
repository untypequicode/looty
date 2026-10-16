package io.github.untypequicode.looty.ui

import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.PopupProperties
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Contact
import io.github.untypequicode.looty.api.MailAddress
import io.github.untypequicode.looty.api.MailMessage
import io.github.untypequicode.looty.api.Uploaded
import io.github.untypequicode.looty.readSharedFile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.add
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
import kotlinx.serialization.json.putJsonObject
import org.jsoup.Jsoup

private fun esc(s: String) = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
private fun MailAddress.formatted(): String = address?.let { a -> name?.takeIf { it.isNotBlank() }?.let { "$it <$a>" } ?: a }.orEmpty()
private fun Contact.formatted(): String = name?.takeIf { it.isNotBlank() }?.let { "$it <$address>" } ?: address
private fun addressOf(r: String) = (Regex("<([^>]+)>").find(r)?.groupValues?.get(1) ?: r).trim().lowercase()
private val EMAIL = Regex("^[^\\s@<>]+@[^\\s@<>]+\\.[^\\s@<>]+$")
private fun looksValid(r: String) = EMAIL.matches(addressOf(r))

/** HTML → texte (partie texte du message, citation « > »). */
fun htmlToText(html: String): String {
    val marked = html
        .replace(Regex("<br\\s*/?>", RegexOption.IGNORE_CASE), "\n")
        .replace(Regex("<li[^>]*>", RegexOption.IGNORE_CASE), "\n- ")
        .replace(Regex("</(p|div|h[1-6]|tr|blockquote|pre|ul|ol)>", RegexOption.IGNORE_CASE), "\n")
    val doc = Jsoup.parse(marked)
    doc.outputSettings().prettyPrint(false)
    return doc.body().wholeText().replace(" ", " ").lines().joinToString("\n") { it.trimEnd() }.replace(Regex("\n{3,}"), "\n\n").trim()
}

/** Destinataires d'un champ : adresses validées + texte en cours de saisie. */
private class Recipients(initial: List<String>) {
    val list: SnapshotStateList<String> = mutableStateListOf<String>().apply { addAll(initial) }
    var text by mutableStateOf("")
    /** Ajoute le texte en cours s'il ressemble à une adresse. */
    fun flush() {
        text.split(',', ';').map { it.trim() }.filter { it.isNotEmpty() }.forEach { if (list.none { x -> addressOf(x) == addressOf(it) }) list.add(it) }
        text = ""
    }
}

/**
 * Écriture d'un message (nouveau, réponse, réponse à tous, transfert) : destinataires avec
 * suggestions, objet, texte mis en forme, pièces jointes, signature du compte ajoutée à l'envoi.
 * Le message cité ou transféré est joint sous le texte, tel quel.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ComposeScreen(state: AppState, mode: String, prefillTo: String, onClose: () -> Unit, onSetupSending: (String) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val senders = state.mailAccounts.data.orEmpty().filter { it.can_send }
    var accountId by remember { mutableStateOf(state.currentMailAccount?.takeIf { it.can_send }?.id ?: senders.firstOrNull()?.id) }
    val account = senders.firstOrNull { it.id == accountId }
    val original: MailMessage? = remember { if (mode == "new") null else state.openMail }
    val wrote = stringResource(R.string.mail_wrote, mailFullDate(original?.date), original?.from?.firstOrNull()?.formatted().orEmpty())
    val fwdHeader = listOf(
        "---------- " + stringResource(R.string.forwarded_header) + " ----------",
        stringResource(R.string.mail_from_field) + " : " + original?.from.orEmpty().joinToString { it.formatted() },
        stringResource(R.string.mail_date) + " : " + mailFullDate(original?.date),
        stringResource(R.string.mail_subject) + " : " + original?.subject.orEmpty(),
        stringResource(R.string.mail_to_field) + " : " + original?.to.orEmpty().joinToString { it.formatted() },
    )

    // Destinataires et objet de départ (mêmes règles que sur le site).
    val init = remember {
        val m = original
        val me = state.mailAccounts.data.orEmpty().map { it.email.lowercase() }.toSet()
        if (m == null || mode == "forward") {
            Triple(if (mode == "new") prefillTo.split(',').map { it.trim() }.filter { it.isNotEmpty() } else emptyList(), emptyList<String>(),
                if (m == null) "" else (m.subject.orEmpty()).let { if (Regex("^(fwd?|tr)\\s*:", RegexOption.IGNORE_CASE).containsMatchIn(it)) it else "Fwd: $it" })
        } else {
            val mine = m.from.any { (it.address ?: "").lowercase() in me }
            var to = (if (mine) m.to else m.reply_to.ifEmpty { m.from }).map { it.formatted() }.filter { it.isNotEmpty() }
            val toAddr = to.map { addressOf(it) }.toSet()
            val cc = if (mode == "replyAll") ((if (mine) emptyList() else m.to) + m.cc).filter { a -> a.address != null && a.address.lowercase() !in me && a.address.lowercase() !in toAddr }.map { it.formatted() }.toMutableList() else mutableListOf()
            if (to.isEmpty() && cc.isNotEmpty()) to = listOf(cc.removeAt(0))
            val subject = m.subject.orEmpty().let { if (Regex("^(re|réf?)\\s*:", RegexOption.IGNORE_CASE).containsMatchIn(it)) it else "Re: $it" }
            Triple(to, cc.toList(), subject)
        }
    }
    val to = remember { Recipients(init.first) }
    val cc = remember { Recipients(init.second) }
    val bcc = remember { Recipients(emptyList()) }
    var showCc by remember { mutableStateOf(init.second.isNotEmpty()) }
    var subject by remember { mutableStateOf(init.third) }
    val editor = remember { DescEditorState(null) {} }
    val quote = remember(original) {
        original?.let { m ->
            val body = m.html ?: "<p>${esc(m.text.orEmpty()).replace("\n", "<br>")}</p>"
            if (mode == "forward") "<p>${fwdHeader.joinToString("<br>") { esc(it) }}</p>$body" else "<p>${esc(wrote)}</p>$body"
        }
    }
    var showQuote by remember { mutableStateOf(mode == "forward") }
    val files = remember { mutableStateListOf<Uploaded>() }
    var uploading by remember { mutableIntStateOf(0) }
    var sending by remember { mutableStateOf(false) }
    var confirmDiscard by remember { mutableStateOf(false) }
    var confirmNoSubject by remember { mutableStateOf(false) }
    var fromMenu by remember { mutableStateOf(false) }
    val snackbar = remember { SnackbarHostState() }

    val dirty = (mode == "new" && (to.list.isNotEmpty() || subject.isNotBlank())) || !editor.isEmpty || files.isNotEmpty() || to.text.isNotBlank()
    fun close() { if (dirty && !sending) confirmDiscard = true else onClose() }
    BackHandler { close() }

    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        uris.forEach { u ->
            uploading++
            scope.launch {
                val f = withContext(Dispatchers.IO) { readSharedFile(ctx, u) }
                val r = f?.let { state.uploadFile(it) }
                when {
                    f == null -> snackbar.showSnackbar(ctx.getString(R.string.err_mail_too_large))
                    r!!.isSuccess -> files.add(r.getOrThrow())
                    else -> snackbar.showSnackbar(errorText(ctx, r.exceptionOrNull()))
                }
                uploading--
            }
        }
    }

    fun send(force: Boolean = false) {
        val acc = account ?: return
        to.flush(); cc.flush(); bcc.flush()
        val all = to.list + cc.list + bcc.list
        if (all.isEmpty()) { scope.launch { snackbar.showSnackbar(ctx.getString(R.string.err_mail_no_recipient)) }; return }
        all.firstOrNull { !looksValid(it) }?.let { bad -> scope.launch { snackbar.showSnackbar(ctx.getString(R.string.err_mail_invalid_address) + " : " + bad) }; return }
        if (uploading > 0) { scope.launch { snackbar.showSnackbar(ctx.getString(R.string.wait_uploads)) }; return }
        if (subject.isBlank() && !force) { confirmNoSubject = true; return }
        sending = true
        val bodyHtml = RichDoc.toHtml(editor.blocks) ?: "<p></p>"
        val sig = acc.signature?.takeIf { it.isNotBlank() }
        val sigHtml = sig?.let { "<p>-- <br>${esc(it).replace("\n", "<br>")}</p>" }.orEmpty()
        val quoteHtml = quote?.let { if (mode == "forward") it else "<blockquote type=\"cite\">$it</blockquote>" }.orEmpty()
        val quoteText = quote?.let { q -> htmlToText(q).let { t -> if (mode == "forward") t else t.lines().joinToString("\n") { "> $it" } } }?.let { "\n\n$it" }.orEmpty()
        val text = htmlToText(bodyHtml) + (sig?.let { "\n\n-- \n$it" }.orEmpty()) + quoteText
        val m = original
        val body = buildJsonObject {
            putJsonArray("to") { to.list.forEach { add(it) } }
            putJsonArray("cc") { cc.list.forEach { add(it) } }
            putJsonArray("bcc") { bcc.list.forEach { add(it) } }
            put("subject", subject.trim())
            put("html", bodyHtml + sigHtml + quoteHtml)
            put("text", text)
            putJsonArray("attachments") { files.forEach { f -> add(buildJsonObject { put("url", f.url); put("name", f.name) }) } }
            if (m != null && mode != "forward") {
                m.message_id?.let { put("in_reply_to", it) } ?: put("in_reply_to", JsonNull)
                putJsonArray("references") { (m.references + listOfNotNull(m.message_id)).takeLast(20).forEach { add(it) } }
                putJsonObject("reply") { put("folder", m.folder); put("uid", m.uid) }
            }
            if (m != null && mode == "forward") putJsonObject("forward") { put("folder", m.folder); put("uid", m.uid) }
        }
        scope.launch {
            state.sendMail(acc.id, body)
                .onSuccess { r -> Toast.makeText(ctx, if (r.warning != null) R.string.mail_sent_no_copy else R.string.mail_sent, Toast.LENGTH_SHORT).show(); onClose() }
                .onFailure { snackbar.showSnackbar(errorText(ctx, it)) }
            sending = false
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(when (mode) { "reply" -> R.string.reply; "replyAll" -> R.string.reply_all; "forward" -> R.string.forward; else -> R.string.compose }), maxLines = 1) },
                navigationIcon = { IconButton(onClick = { close() }) { Icon(Icons.Outlined.Close, stringResource(R.string.cancel)) } },
                actions = {
                    if (account != null) {
                        IconButton(onClick = { pick.launch(arrayOf("*/*")) }) { Icon(Icons.Outlined.AttachFile, stringResource(R.string.add_file)) }
                        if (sending) Box(Modifier.size(48.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp) }
                        else IconButton(onClick = { send() }) { Icon(Icons.AutoMirrored.Filled.Send, stringResource(R.string.send), tint = MaterialTheme.colorScheme.primary) }
                    }
                },
            )
        },
        bottomBar = { DescEditorToolbar(editor) },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        if (account == null) {
            // Aucun compte ne peut envoyer : configurer le serveur d'envoi.
            Column(Modifier.padding(padding).fillMaxSize().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
                EmptyState(Icons.Outlined.ForwardToInbox, stringResource(R.string.setup_sending_first))
                state.currentMailAccount?.let { a -> Button(onClick = { onSetupSending(a.id) }) { Text(stringResource(R.string.edit_mail_account)) } }
            }
            return@Scaffold
        }
        Column(Modifier.padding(padding).fillMaxSize().verticalScroll(rememberScrollState()).imePadding().padding(bottom = 24.dp)) {
            // Expéditeur
            Box(Modifier.padding(horizontal = 16.dp)) {
                Row(Modifier.fillMaxWidth().clickable(enabled = senders.size > 1) { fromMenu = true }.padding(vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.mail_from_field), color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.width(48.dp))
                    Text(account.display_name?.takeIf { it.isNotBlank() }?.let { "$it <${account.email}>" } ?: account.email, Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (senders.size > 1) Icon(Icons.Filled.ArrowDropDown, null)
                }
                DropdownMenu(fromMenu, { fromMenu = false }) {
                    senders.forEach { a -> DropdownMenuItem(text = { Text(a.email) }, onClick = { fromMenu = false; accountId = a.id }, leadingIcon = { RadioButton(a.id == accountId, null) }) }
                }
            }
            HorizontalDivider()
            val toFocus = remember { FocusRequester() }
            // Nouveau message, transfert : on commence par les destinataires ; réponse : par le texte.
            LaunchedEffect(Unit) {
                if (mode == "new" || mode == "forward") runCatching { toFocus.requestFocus() }
                else editor.blocks.firstOrNull()?.let { editor.pendingFocus = it.id to 0 }
            }
            RecipientField(state, stringResource(R.string.mail_to_field), to, toFocus, trailing = {
                if (!showCc) TextButton(onClick = { showCc = true }) { Text(stringResource(R.string.show_cc)) }
            })
            if (showCc) {
                RecipientField(state, stringResource(R.string.mail_cc_field), cc, null)
                RecipientField(state, stringResource(R.string.mail_bcc_field), bcc, null)
            }
            TextField(subject, { subject = it.replace("\n", " ") }, singleLine = true, placeholder = { Text(stringResource(R.string.mail_subject)) },
                colors = TextFieldDefaults.colors(focusedContainerColor = Color.Transparent, unfocusedContainerColor = Color.Transparent),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next), modifier = Modifier.fillMaxWidth())
            DescriptionEditor(state, editor, Modifier.fillMaxWidth().heightIn(min = 160.dp).padding(16.dp), placeholder = stringResource(R.string.mail_body_placeholder))
            account.signature?.takeIf { it.isNotBlank() }?.let { sig ->
                Text("-- \n$sig", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.padding(horizontal = 16.dp))
            }
            // Pièces jointes
            if (files.isNotEmpty() || uploading > 0 || (mode == "forward" && original?.attachments?.isNotEmpty() == true)) {
                SectionTitle(stringResource(R.string.attachments))
                if (uploading > 0) LinearProgressIndicator(Modifier.fillMaxWidth().padding(horizontal = 16.dp))
                files.forEach { f ->
                    ListItem(headlineContent = { Text(f.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        supportingContent = { Text(android.text.format.Formatter.formatShortFileSize(ctx, f.size)) },
                        leadingContent = { Icon(Icons.Outlined.AttachFile, null) },
                        trailingContent = { IconButton(onClick = { files.remove(f) }) { Icon(Icons.Outlined.Close, stringResource(R.string.delete)) } })
                }
                if (mode == "forward") original?.attachments?.takeIf { it.isNotEmpty() }?.let { atts ->
                    atts.forEach { a -> ListItem(headlineContent = { Text(a.name, maxLines = 1, overflow = TextOverflow.Ellipsis) }, leadingContent = { Icon(Icons.Outlined.AttachFile, null) },
                        supportingContent = { Text(stringResource(R.string.forward_attachments)) }) }
                }
            }
            // Message cité ou transféré
            quote?.let { q ->
                TextButton(onClick = { showQuote = !showQuote }, modifier = Modifier.padding(horizontal = 8.dp)) {
                    Icon(if (showQuote) Icons.Outlined.ExpandLess else Icons.Outlined.ExpandMore, null); Spacer(Modifier.width(6.dp)); Text(stringResource(R.string.quoted_text))
                }
                if (showQuote) HtmlBlocks(state, q, Modifier.padding(horizontal = 16.dp))
            }
        }
    }

    if (confirmDiscard) AlertDialog(
        onDismissRequest = { confirmDiscard = false },
        title = { Text(stringResource(R.string.discard_title)) },
        confirmButton = { TextButton(onClick = { confirmDiscard = false; onClose() }) { Text(stringResource(R.string.discard), color = MaterialTheme.colorScheme.error) } },
        dismissButton = { TextButton(onClick = { confirmDiscard = false }) { Text(stringResource(R.string.cancel)) } },
    )
    if (confirmNoSubject) AlertDialog(
        onDismissRequest = { confirmNoSubject = false },
        title = { Text(stringResource(R.string.no_subject_title)) },
        confirmButton = { TextButton(onClick = { confirmNoSubject = false; send(force = true) }) { Text(stringResource(R.string.send)) } },
        dismissButton = { TextButton(onClick = { confirmNoSubject = false }) { Text(stringResource(R.string.cancel)) } },
    )
}

/** Champ de destinataires : adresses en pastilles, suggestions (contacts, membres) pendant la saisie. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RecipientField(state: AppState, label: String, r: Recipients, focus: FocusRequester?, trailing: @Composable () -> Unit = {}) {
    var suggestions by remember { mutableStateOf<List<Contact>>(emptyList()) }
    var focused by remember { mutableStateOf(false) }
    LaunchedEffect(r.text) {
        val q = r.text.trim()
        if (q.length < 2) { suggestions = emptyList(); return@LaunchedEffect }
        delay(250)
        suggestions = state.mailContacts(q).filter { c -> r.list.none { addressOf(it) == c.address.lowercase() } }.take(6)
    }
    Column {
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.width(48.dp))
            FlowRow(Modifier.weight(1f).padding(vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp),
                itemVerticalAlignment = Alignment.CenterVertically) {
                r.list.forEach { x ->
                    val ok = looksValid(x)
                    InputChip(selected = false, onClick = { r.list.remove(x) }, label = { Text(x.substringBefore('<').trim().ifBlank { x }, maxLines = 1) },
                        trailingIcon = { Icon(Icons.Outlined.Close, stringResource(R.string.delete), Modifier.size(16.dp)) },
                        colors = if (ok) InputChipDefaults.inputChipColors() else InputChipDefaults.inputChipColors(containerColor = MaterialTheme.colorScheme.errorContainer))
                }
                Box(Modifier.weight(1f).widthIn(min = 120.dp)) {
                    androidx.compose.foundation.text.BasicTextField(
                        value = r.text,
                        onValueChange = { v -> if (v.endsWith(',') || v.endsWith(';') || (v.endsWith(' ') && looksValid(v.trim()))) { r.text = v.trimEnd(',', ';', ' '); r.flush() } else r.text = v },
                        singleLine = true, textStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onSurface),
                        cursorBrush = androidx.compose.ui.graphics.SolidColor(MaterialTheme.colorScheme.primary),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, imeAction = ImeAction.Next),
                        keyboardActions = KeyboardActions(onNext = { r.flush() }),
                        modifier = Modifier.fillMaxWidth().padding(vertical = 10.dp).let { if (focus != null) it.focusRequester(focus) else it }
                            .onFocusChanged { f -> if (focused && !f.isFocused) r.flush(); focused = f.isFocused },
                    )
                    DropdownMenu(expanded = focused && suggestions.isNotEmpty(), onDismissRequest = { suggestions = emptyList() }, properties = PopupProperties(focusable = false)) {
                        suggestions.forEach { c ->
                            DropdownMenuItem(
                                text = { Column { Text(c.name?.takeIf { it.isNotBlank() } ?: c.address, maxLines = 1); if (!c.name.isNullOrBlank()) Text(c.address, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) } },
                                leadingIcon = { Avatar(c.name ?: c.address, 28.dp) },
                                onClick = { r.list.add(c.formatted()); r.text = ""; suggestions = emptyList() },
                            )
                        }
                    }
                }
            }
            trailing()
        }
        HorizontalDivider()
    }
}
