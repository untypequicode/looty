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
