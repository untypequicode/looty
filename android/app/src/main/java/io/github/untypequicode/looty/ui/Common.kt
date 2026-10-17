package io.github.untypequicode.looty.ui

import android.content.Context
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.WebActivity
import io.github.untypequicode.looty.api.Notification
import kotlinx.serialization.json.jsonPrimitive
import java.time.Duration
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

/** Liens vers les écrans web de l'espace. */
object Paths {
    fun doc(ws: String, id: String) = "/w/$ws/doc/$id"
    fun task(ws: String, board: String, id: String) = "/w/$ws/board/$board?task=$id"
    fun board(ws: String, id: String) = "/w/$ws/board/$id"
    fun screen(ws: String, name: String) = "/w/$ws/$name"
    fun notification(ws: String, n: Notification): String {
        val (kind, id) = n.target.split(":", limit = 2).let { it[0] to it.getOrElse(1) { "" } }
        return when (kind) {
            "task" -> n.data["board_id"]?.jsonPrimitive?.content?.let { task(ws, it, id) } ?: "/w/$ws/task/$id"
            "page" -> doc(ws, id)
            "board" -> board(ws, id)
            "mail" -> "/w/$ws/mail?account=$id"
            else -> "/w/$ws/inbox"
        }
    }
}

fun openWeb(ctx: Context, path: String) = WebActivity.open(ctx, path)

/** Avatar : initiales sur une couleur stable par personne. */
@Composable
fun Avatar(name: String?, size: Dp = 32.dp) {
    val label = name?.trim()?.split(Regex("\\s+"))?.take(2)?.joinToString("") { it.take(1).uppercase() }?.ifEmpty { "?" } ?: "?"
    val hues = listOf(0xFF5E6AD2, 0xFF2A78D6, 0xFF1BAF7A, 0xFFEB6834, 0xFFE87BA4, 0xFF4A3AA7, 0xFFEDA100)
    val c = Color(hues[(name ?: "").hashCode().mod(hues.size)])
    Box(Modifier.size(size).clip(CircleShape).background(c), contentAlignment = Alignment.Center) {
        Text(label, color = Color.White, fontSize = (size.value * 0.38f).sp, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
fun EmptyState(icon: ImageVector, text: String, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth().padding(vertical = 48.dp, horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Icon(icon, null, Modifier.size(40.dp), tint = MaterialTheme.colorScheme.outline)
        Spacer(Modifier.height(12.dp))
        Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
    }
}

@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier, action: (@Composable () -> Unit)? = null) {
    Row(modifier.fillMaxWidth().padding(start = 20.dp, end = 8.dp, top = 20.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.weight(1f))
        action?.invoke()
    }
}

/** « il y a 5 min », « hier »… */
fun relativeTime(iso: String, ctx: Context): String {
    val t = runCatching { Instant.parse(iso) }.getOrNull() ?: return ""
    val d = Duration.between(t, Instant.now())
    val fr = ctx.resources.configuration.locales[0].language == "fr"
    return when {
        d.toMinutes() < 1 -> if (fr) "à l’instant" else "just now"
        d.toMinutes() < 60 -> if (fr) "il y a ${d.toMinutes()} min" else "${d.toMinutes()} min ago"
        d.toHours() < 24 -> if (fr) "il y a ${d.toHours()} h" else "${d.toHours()} h ago"
        d.toDays() < 7 -> if (fr) "il y a ${d.toDays()} j" else "${d.toDays()} d ago"
        else -> t.atZone(ZoneId.systemDefault()).toLocalDate().format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM))
    }
}

fun localDate(iso: String?): LocalDate? = iso?.let { runCatching { Instant.parse(it).atZone(ZoneId.systemDefault()).toLocalDate() }.getOrNull() }

@Composable
fun dayLabel(d: LocalDate): String = when (d) {
    LocalDate.now() -> stringResource(R.string.today)
    LocalDate.now().plusDays(1) -> stringResource(R.string.tomorrow)
    else -> d.format(DateTimeFormatter.ofPattern("d MMM"))
}

@Composable
fun notificationText(n: Notification): String {
    val who = n.actor_name ?: "?"
    return when (n.kind) {
        "mention" -> stringResource(R.string.n_mention, who)
        "assigned" -> stringResource(R.string.n_assigned, who)
        "comment" -> stringResource(R.string.n_comment, who)
        "due_soon" -> stringResource(R.string.n_due_soon)
        "completed" -> stringResource(R.string.n_completed, who)
        "mail" -> stringResource(R.string.n_mail)
        else -> stringResource(R.string.n_other)
    }
}

/**
 * À appeler dans le contenu d'une feuille modale : sa fenêtre de dialogue ajoute sinon un
 * voile clair sur la barre de navigation par gestes (comme l'activité, qui le désactive).
 */
@Composable
fun NoNavScrim() {
    val view = androidx.compose.ui.platform.LocalView.current
    androidx.compose.runtime.LaunchedEffect(view) {
        if (android.os.Build.VERSION.SDK_INT >= 29) {
            val provider = (view.parent as? androidx.compose.ui.window.DialogWindowProvider) ?: (view as? androidx.compose.ui.window.DialogWindowProvider)
            provider?.window?.isNavigationBarContrastEnforced = false
        }
    }
}

/** Lignes de liste dans une feuille modale : fond de la feuille, pas celui de l'écran. */
@Composable
fun sheetItemColors() = ListItemDefaults.colors(containerColor = Color.Transparent)

/** Code d'erreur du serveur → phrase pour l'utilisateur (les codes inconnus donnent un message général). */
fun errorText(ctx: Context, code: String?): String {
    val c = code.orEmpty().substringBefore(':').trim()
    val res = when (c) {
        "forbidden", "http_403" -> R.string.error_forbidden
        "offline" -> R.string.error_offline
        "mail_unreachable" -> R.string.err_mail_unreachable
        "mail_private_address" -> R.string.err_mail_private_address
        "mail_tls_failed" -> R.string.err_mail_tls_failed
        "mail_starttls_failed" -> R.string.err_mail_starttls_failed
        "mail_auth_failed" -> R.string.err_mail_auth_failed
        "mail_timeout" -> R.string.err_mail_timeout
        "mail_imap_error" -> R.string.err_mail_imap_error
        "mail_secret_unreadable" -> R.string.err_mail_secret_unreadable
        "mail_invalid_host" -> R.string.err_mail_invalid_host
        "mail_invalid_webmail" -> R.string.err_mail_invalid_webmail
        "mail_too_many" -> R.string.err_mail_too_many
        "mail_unreadable" -> R.string.err_mail_unreadable
        "mail_smtp_auth_failed" -> R.string.err_mail_smtp_auth_failed
        "mail_smtp_unreachable" -> R.string.err_mail_smtp_unreachable
        "mail_smtp_error" -> R.string.err_mail_smtp_error
        "mail_no_smtp" -> R.string.err_mail_no_smtp
        "mail_no_recipient" -> R.string.err_mail_no_recipient
        "mail_too_many_recipients" -> R.string.err_mail_too_many_recipients
        "mail_invalid_address" -> R.string.err_mail_invalid_address
        "mail_invalid_from" -> R.string.err_mail_invalid_from
        "mail_too_large" -> R.string.err_mail_too_large
        "mail_invalid_attachment" -> R.string.err_mail_invalid_attachment
        "mail_invalid_message" -> R.string.err_mail_invalid_message
        "mail_invalid_folder" -> R.string.err_mail_invalid_folder
        "mail_folder_failed" -> R.string.err_mail_folder_failed
        "mail_move_failed" -> R.string.err_mail_move_failed
        "moodle_invalid_url" -> R.string.err_moodle_invalid_url
        "moodle_unreachable" -> R.string.err_moodle_unreachable
        "moodle_not_moodle" -> R.string.err_moodle_not_moodle
        "moodle_ws_disabled" -> R.string.err_moodle_ws_disabled
        "moodle_auth_failed" -> R.string.err_moodle_auth_failed
        "moodle_invalid_token" -> R.string.err_moodle_invalid_token
        "moodle_not_connected" -> R.string.err_moodle_not_connected
        "moodle_bad_response" -> R.string.err_moodle_bad_response
        "moodle_error" -> R.string.err_moodle_error
        "moodle_http_error" -> R.string.err_moodle_http_error
        "moodle_no_calendar_export" -> R.string.err_moodle_no_calendar_export
        "moodle_timeout" -> R.string.err_moodle_timeout
        else -> R.string.error_generic
    }
    val detail = code.orEmpty().substringAfter(':', "").trim()
    return ctx.getString(res) + if (c == "mail_invalid_address" && detail.isNotEmpty()) " : $detail" else ""
}

/** Message d'erreur d'une exception (code du serveur, ou serveur injoignable). */
fun errorText(ctx: Context, e: Throwable?): String =
    errorText(ctx, (e as? io.github.untypequicode.looty.api.ApiException)?.code ?: "offline")
