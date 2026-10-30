package io.github.untypequicode.looty.widget

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.ColorFilter
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.Image
import androidx.glance.ImageProvider
import androidx.glance.LocalContext
import androidx.glance.action.Action
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.updateAll
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import io.github.untypequicode.looty.MainActivity
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Api

/** Couleurs d'un widget : thème de l'appareil (Material You) déclinés selon le style choisi. */
class Palette(val bg: Color, val fg: Color, val soft: Color, val accent: Color, val onAccent: Color, val chip: Color, val warn: Color)

@Composable
fun palette(style: WidgetStyle): Palette {
    val ctx = LocalContext.current
    val c = GlanceTheme.colors
    fun col(p: androidx.glance.unit.ColorProvider) = p.getColor(ctx)
    val a = style.opacity / 100f
    return when (style.look) {
        "surface" -> Palette(col(c.widgetBackground).copy(alpha = a), col(c.onSurface), col(c.onSurfaceVariant), col(c.primary), col(c.onPrimary), col(c.secondaryContainer), col(c.error))
        "accent" -> Palette(col(c.primary).copy(alpha = a), col(c.onPrimary), col(c.onPrimary).copy(alpha = .78f), col(c.onPrimary), col(c.primary), col(c.onPrimary).copy(alpha = .18f), col(c.errorContainer))
        "contrast" -> Palette(col(c.inverseSurface).copy(alpha = a), col(c.inverseOnSurface), col(c.inverseOnSurface).copy(alpha = .72f), col(c.inversePrimary), col(c.inverseSurface), col(c.inverseOnSurface).copy(alpha = .14f), col(c.errorContainer))
        else -> Palette(col(c.secondaryContainer).copy(alpha = a), col(c.onSecondaryContainer), col(c.onSecondaryContainer).copy(alpha = .72f), col(c.primary), col(c.onPrimary), col(c.surface).copy(alpha = .55f), col(c.error))
    }
}

fun cp(c: Color) = ColorProvider(c)

/** Rayon des grands conteneurs (très arrondi en Material 3 Expressive). */
fun outer(style: WidgetStyle): Dp = if (style.round) 36.dp else 22.dp
fun inner(style: WidgetStyle): Dp = if (style.round) 22.dp else 14.dp

val TaskKey = ActionParameters.Key<String>("task")

/** Lien ouvert dans l'application : looty://app/<écran> ou looty://w/…. */
fun openApp(ctx: Context, link: String? = null): Action {
    val intent = if (link == null) Intent(ctx, MainActivity::class.java) else Intent(Intent.ACTION_VIEW, Uri.parse(link)).setPackage(ctx.packageName)
    return actionStartActivity(intent)
}

@Composable
fun Icon(res: Int, color: Color, size: Dp = 20.dp, modifier: GlanceModifier = GlanceModifier) =
    Image(ImageProvider(res), null, modifier.size(size), colorFilter = ColorFilter.tint(cp(color)))

/** Bouton plein, arrondi en pilule (icône + texte facultatif). */
@Composable
fun PillButton(res: Int, label: String?, p: Palette, action: Action, modifier: GlanceModifier = GlanceModifier) {
    Row(
        modifier.background(cp(p.accent)).cornerRadius(100.dp).padding(horizontal = if (label == null) 10.dp else 14.dp, vertical = 9.dp).clickable(action),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(res, p.onAccent, 18.dp)
        if (label != null) {
            Spacer(GlanceModifier.width(6.dp))
            Text(label, maxLines = 1, style = TextStyle(color = cp(p.onAccent), fontSize = 13.sp, fontWeight = FontWeight.Medium))
        }
    }
}

/** Pastille d'information (échéance, nombre…). */
@Composable
fun Chip(text: String, p: Palette, color: Color = p.fg, bg: Color = p.chip) {
    Box(GlanceModifier.background(cp(bg)).cornerRadius(100.dp).padding(horizontal = 9.dp, vertical = 3.dp)) {
        Text(text, maxLines = 1, style = TextStyle(color = cp(color), fontSize = 12.sp, fontWeight = FontWeight.Medium))
    }
}

/** Les widgets posés sur l'écran d'accueil sont tous rafraîchis ensemble. */
object Widgets {
    suspend fun updateAll(ctx: Context) {
        runCatching { LootyWidget().updateAll(ctx) }
        runCatching { AgendaWidget().updateAll(ctx) }
        runCatching { TodayWidget().updateAll(ctx) }
        runCatching { ShortcutsWidget().updateAll(ctx) }
    }

    /** Style d'un widget posé, d'après son identifiant Glance. */
    suspend fun styleOf(ctx: Context, id: androidx.glance.GlanceId): WidgetStyle =
        WidgetPrefs.load(ctx, GlanceAppWidgetManager(ctx).getAppWidgetId(id))
}

/** Coche une tâche depuis le widget : appel direct au serveur, puis rafraîchissement. */
class ToggleTaskAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: androidx.glance.GlanceId, parameters: ActionParameters) {
        val id = parameters[TaskKey] ?: return
        runCatching { Api(context).send("PUT", "/api/boards/tasks/$id/toggle_validation") }
        Widgets.updateAll(context)
    }
}

fun toggle(taskId: String): Action = actionRunCallback<ToggleTaskAction>(actionParametersOf(TaskKey to taskId))
