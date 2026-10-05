package io.github.untypequicode.looty.ui

import android.annotation.SuppressLint
import android.content.Intent
import android.net.Uri
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.outlined.*
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import io.github.untypequicode.looty.AppState
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.MailAddress
import io.github.untypequicode.looty.api.MailMessage
import io.github.untypequicode.looty.api.MailSummary
import androidx.compose.ui.text.withLink
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

private fun mailTime(iso: String?): String {
    val t = runCatching { Instant.parse(iso) }.getOrNull()?.atZone(ZoneId.systemDefault()) ?: return ""
    return when {
        t.toLocalDate() == LocalDate.now() -> t.format(DateTimeFormatter.ofPattern("HH:mm"))
        t.year == LocalDate.now().year -> t.format(DateTimeFormatter.ofPattern("d MMM"))
        else -> t.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT))
    }
}

private fun openUrl(ctx: android.content.Context, url: String) {
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
}

/** Boîte de réception (lecture seule) : comptes, tous / non lus, lu / non lu. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MailScreen(state: AppState, onBack: () -> Unit, onMessage: (Long) -> Unit) {
    val ctx = LocalContext.current
    val accounts = state.mailAccounts.data
    val account = state.currentMailAccount
    var accountMenu by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { state.loadMailAccounts() }
    LaunchedEffect(account?.id) { if (account != null) state.loadMail() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Box {
                        Row(Modifier.clip(RoundedCornerShape(8.dp)).clickable(enabled = (accounts?.size ?: 0) > 1) { accountMenu = true }, verticalAlignment = Alignment.CenterVertically) {
                            Column {
                                Text(account?.label ?: stringResource(R.string.mail), maxLines = 1, overflow = TextOverflow.Ellipsis)
                                account?.email?.takeIf { it.isNotBlank() }?.let { Text(it, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1) }
                            }
                            if ((accounts?.size ?: 0) > 1) Icon(Icons.Filled.ArrowDropDown, null)
                        }
                        DropdownMenu(accountMenu, { accountMenu = false }) {
                            accounts.orEmpty().forEach { a ->
                                DropdownMenuItem(
                                    text = { Text(a.label) },
                                    trailingIcon = { if (a.unread > 0) Badge { Text(a.unread.toString()) } },
                                    leadingIcon = { RadioButton(a.id == account?.id, null) },
                                    onClick = { accountMenu = false; state.selectMailAccount(a.id) },
                                )
                            }
                        }
                    }
                },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    if (account != null) {
                        account.webmail_url?.let { u -> IconButton(onClick = { openUrl(ctx, u) }) { Icon(Icons.Outlined.OpenInBrowser, stringResource(R.string.open_webmail)) } }
                        IconButton(onClick = { menu = true }) { Icon(Icons.Outlined.MoreVert, null) }
                        DropdownMenu(menu, { menu = false }) {
                            DropdownMenuItem(text = { Text(stringResource(R.string.mark_all_read)) }, leadingIcon = { Icon(Icons.Outlined.DoneAll, null) }, enabled = account.unread > 0,
                                onClick = { menu = false; state.readAllMail() })
                            DropdownMenuItem(text = { Text(stringResource(R.string.manage_mail_accounts)) }, leadingIcon = { Icon(Icons.Outlined.Settings, null) },
                                onClick = { menu = false; state.ws?.let { openWeb(ctx, Paths.screen(it, "mail")) } })
                        }
                    }
                },
            )
        },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            when {
                accounts == null && state.mailAccounts.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                accounts == null -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic))
                account == null -> Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                    EmptyState(Icons.Outlined.Mail, stringResource(R.string.no_mail_account))
                    Button(onClick = { state.ws?.let { openWeb(ctx, Paths.screen(it, "mail")) } }) { Text(stringResource(R.string.set_up_mail)) }
                }
                else -> {
                    Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilterChip(!state.mailUnreadOnly, { state.setMailFilter(false) }, label = { Text(stringResource(R.string.mail_all)) })
                        FilterChip(state.mailUnreadOnly, { state.setMailFilter(true) }, label = { Text(if (account.unread > 0) stringResource(R.string.mail_unread_n, account.unread) else stringResource(R.string.mail_unread)) })
                    }
                    account.last_error?.let {
                        Surface(color = MaterialTheme.colorScheme.errorContainer, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)) {
                            Text(stringResource(R.string.mail_error, it), color = MaterialTheme.colorScheme.onErrorContainer, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(12.dp))
                        }
                    }
                    MailList(state, onMessage)
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MailList(state: AppState, onMessage: (Long) -> Unit) {
    val list = state.mailList.data
    val listState = rememberLazyListState()
    val atEnd by remember { derivedStateOf { listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index?.let { it >= listState.layoutInfo.totalItemsCount - 3 } ?: false } }
    LaunchedEffect(atEnd, state.mailHasMore) { if (atEnd && state.mailHasMore) state.loadMoreMail() }
    PullToRefreshBox(isRefreshing = state.mailList.loading && list != null, onRefresh = { state.refreshMail() }, modifier = Modifier.fillMaxSize()) {
        when {
            list == null && state.mailList.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
            list == null -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic) + state.mailList.error?.let { " ($it)" }.orEmpty())
            list.isEmpty() -> LazyColumn(Modifier.fillMaxSize()) { item { EmptyState(Icons.Outlined.Inbox, stringResource(if (state.mailUnreadOnly) R.string.no_unread_mail else R.string.no_mail)) } }
            else -> LazyColumn(Modifier.fillMaxSize(), state = listState, contentPadding = PaddingValues(bottom = 24.dp)) {
                items(list, key = { it.uid }) { m -> MailRow(m, onClick = { onMessage(m.uid) }, onToggle = { state.setSeen(m.uid, !m.seen) }) }
                if (state.mailLoadingMore) item { Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(Modifier.size(24.dp)) } }
            }
        }
    }
}

/** Glisser vers la droite : lu / non lu. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun MailRow(m: MailSummary, onClick: () -> Unit, onToggle: () -> Unit) {
    // Refuser l'état « balayé » : la ligne revient en place après l'action.
    val toggle by rememberUpdatedState(onToggle)
    @Suppress("DEPRECATION")
    val swipe = rememberSwipeToDismissBoxState(confirmValueChange = { if (it == SwipeToDismissBoxValue.StartToEnd) toggle(); false })
    SwipeToDismissBox(
        swipe,
        enableDismissFromEndToStart = false,
        backgroundContent = {
            Row(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.secondaryContainer).padding(horizontal = 24.dp), verticalAlignment = Alignment.CenterVertically) {
                Icon(if (m.seen) Icons.Outlined.MarkEmailUnread else Icons.Outlined.MarkEmailRead, null, tint = MaterialTheme.colorScheme.onSecondaryContainer)
                Spacer(Modifier.width(12.dp))
                Text(stringResource(if (m.seen) R.string.mark_unread else R.string.mark_read), color = MaterialTheme.colorScheme.onSecondaryContainer)
            }
        },
    ) {
        val weight = if (m.seen) FontWeight.Normal else FontWeight.Bold
        ListItem(
            headlineContent = {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(m.from_name.ifBlank { m.from_addr }, Modifier.weight(1f), fontWeight = weight, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (m.has_attachments) Icon(Icons.Outlined.AttachFile, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(mailTime(m.sent_at), style = MaterialTheme.typography.labelMedium, fontWeight = weight,
                        color = if (m.seen) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 8.dp))
                }
            },
            supportingContent = {
                Column {
                    Text(m.subject.ifBlank { stringResource(R.string.no_subject) }, fontWeight = weight, color = MaterialTheme.colorScheme.onSurface, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (m.snippet.isNotBlank()) Text(m.snippet, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
            },
            leadingContent = { Avatar(m.from_name.ifBlank { m.from_addr }, 40.dp) },
            modifier = Modifier.clickable(onClick = onClick),
        )
    }
}

private fun MailAddress.label() = name?.takeIf { it.isNotBlank() } ?: address.orEmpty()

/** Lecture d'un message : HTML nettoyé par le serveur, sans JavaScript, images distantes bloquées. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MessageScreen(state: AppState, uid: Long, onBack: () -> Unit) {
    val ctx = LocalContext.current
    var msg by remember { mutableStateOf<MailMessage?>(null) }
    var failed by remember { mutableStateOf(false) }
    var images by remember { mutableStateOf(false) }
    val summary = state.mailList.data?.firstOrNull { it.uid == uid }
    LaunchedEffect(uid) { msg = state.readMail(uid); failed = msg == null }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {},
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
                actions = {
                    IconButton(onClick = { state.setSeen(uid, false); onBack() }) { Icon(Icons.Outlined.MarkEmailUnread, stringResource(R.string.mark_unread)) }
                    state.currentMailAccount?.webmail_url?.let { u -> IconButton(onClick = { openUrl(ctx, u) }) { Icon(Icons.Outlined.OpenInBrowser, stringResource(R.string.open_webmail)) } }
                },
            )
        },
    ) { padding ->
        val m = msg
        Column(Modifier.padding(padding).fillMaxSize()) {
            Text((m?.subject ?: summary?.subject)?.ifBlank { null } ?: stringResource(R.string.no_subject), style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp))
            val from = m?.from?.firstOrNull()
            val name = from?.label() ?: summary?.from_name?.ifBlank { summary.from_addr }
            ListItem(
                headlineContent = { Text(name.orEmpty(), fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                supportingContent = {
                    Column {
                        from?.address?.takeIf { it != name }?.let { Text(it, maxLines = 1, overflow = TextOverflow.Ellipsis) }
                        if (m != null && m.to.isNotEmpty()) Text(stringResource(R.string.mail_to, m.to.joinToString { it.label() }), maxLines = 2, overflow = TextOverflow.Ellipsis)
                        if (m != null && m.cc.isNotEmpty()) Text(stringResource(R.string.mail_cc, m.cc.joinToString { it.label() }), maxLines = 2, overflow = TextOverflow.Ellipsis)
                    }
                },
                leadingContent = { Avatar(name, 40.dp) },
                trailingContent = { Text(mailTime(m?.date ?: summary?.sent_at), style = MaterialTheme.typography.labelMedium) },
            )
            if (m != null && m.attachments.isNotEmpty()) {
                Row(Modifier.fillMaxWidth().horizontalScrollCompat().padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    m.attachments.forEach { a ->
                        AssistChip(onClick = { state.currentMailAccount?.webmail_url?.let { openUrl(ctx, it) } }, label = { Text("${a.name} · ${android.text.format.Formatter.formatShortFileSize(ctx, a.size)}", maxLines = 1) },
                            leadingIcon = { Icon(Icons.Outlined.AttachFile, null, Modifier.size(18.dp)) })
                    }
                }
            }
            val html = m?.html
            if (html != null && !images && Regex("<img[^>]+src=[\"']?https?:", RegexOption.IGNORE_CASE).containsMatchIn(html)) {
                Surface(color = MaterialTheme.colorScheme.surfaceContainerHigh, shape = RoundedCornerShape(12.dp), modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)) {
                    Row(Modifier.padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Outlined.HideImage, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                        Spacer(Modifier.width(8.dp))
                        Text(stringResource(R.string.images_blocked), Modifier.weight(1f), style = MaterialTheme.typography.bodySmall)
                        TextButton(onClick = { images = true }) { Text(stringResource(R.string.show_images)) }
                    }
                }
            }
            HorizontalDivider(Modifier.padding(top = 8.dp))
            when {
                m == null && !failed -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { LoadingIndicatorCompat() }
                m == null -> EmptyState(Icons.Outlined.CloudOff, stringResource(R.string.error_generic))
                html != null -> MailHtml(html, images, Modifier.fillMaxSize())
                else -> SelectionContainer {
                    Text(linkify(m.text.orEmpty(), MaterialTheme.colorScheme.primary), Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
}

@Composable
private fun Modifier.horizontalScrollCompat(): Modifier = this.then(Modifier.horizontalScroll(rememberScrollState()))


@SuppressLint("SetJavaScriptEnabled")
@Composable
private fun MailHtml(html: String, images: Boolean, modifier: Modifier) {
    // Fond blanc comme un client de messagerie : les messages HTML supposent souvent un fond clair.
    val doc = """<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="referrer" content="no-referrer">
        <style>body{margin:12px;font-family:sans-serif;font-size:15px;line-height:1.45;color:#1a1a1a;background:#fff;word-wrap:break-word}img{max-width:100%;height:auto}table{max-width:100%!important}pre{white-space:pre-wrap}</style>
        </head><body>$html</body></html>"""
    AndroidView(
        factory = { c ->
            WebView(c).apply {
                settings.javaScriptEnabled = false
                settings.allowFileAccess = false
                settings.allowContentAccess = false
                settings.domStorageEnabled = false
                settings.loadWithOverviewMode = true
                settings.useWideViewPort = true
                settings.builtInZoomControls = true
                settings.displayZoomControls = false
                setBackgroundColor(android.graphics.Color.WHITE)
                webViewClient = object : WebViewClient() {
                    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                        openUrl(view.context, request.url.toString())
                        return true
                    }
                }
            }
        },
        update = { w ->
            w.settings.blockNetworkImage = !images
            w.settings.blockNetworkLoads = !images
            val key = "$images$doc"
            if (w.tag != key) { w.tag = key; w.loadDataWithBaseURL(null, doc, "text/html", "utf-8", null) }
        },
        modifier = modifier,
    )
}

private val URL_RE = Regex("""https?://[^\s<>"')\]]+""")

/** Texte brut : adresses web cliquables (ouvertes dans le navigateur). */
private fun linkify(text: String, color: androidx.compose.ui.graphics.Color): androidx.compose.ui.text.AnnotatedString =
    androidx.compose.ui.text.buildAnnotatedString {
        var last = 0
        val style = androidx.compose.ui.text.TextLinkStyles(androidx.compose.ui.text.SpanStyle(color = color, textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline))
        URL_RE.findAll(text).forEach { m ->
            append(text.substring(last, m.range.first))
            withLink(androidx.compose.ui.text.LinkAnnotation.Url(m.value.trimEnd('.', ','), style)) { append(m.value) }
            last = m.range.last + 1
        }
        append(text.substring(last))
    }
