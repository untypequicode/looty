package io.github.untypequicode.looty.widget

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.LocalSize
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.size
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import io.github.untypequicode.looty.R
import java.time.LocalDate
import java.time.format.DateTimeFormatter

/**
 * Widget « À faire » : mes tâches, chacune dans sa carte arrondie. Cocher une tâche la termine
 * sans ouvrir l'application ; toucher la carte l'ouvre ; « + » ouvre l'ajout rapide.
 * Apparence réglable (style, transparence, nombre de tâches) : appui long → Réglages.
 */
class LootyWidget : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val data = WidgetData.load(context)
        val style = Widgets.styleOf(context, id)
        provideContent { GlanceTheme { Content(context, data, style) } }
    }

    @Composable
    private fun Content(ctx: Context, data: WidgetData, style: WidgetStyle) {
        val p = palette(style)
        val size = LocalSize.current
        val compact = size.height < 140.dp
        val rowH = 46.dp
        val fit = (((size.height - (if (compact) 64.dp else 74.dp)) / rowH).toInt()).coerceIn(1, style.items)
        Column(GlanceModifier.fillMaxSize().background(cp(p.bg)).cornerRadius(outer(style)).padding(14.dp)) {
            Row(GlanceModifier.fillMaxWidth().padding(bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(GlanceModifier.size(34.dp).background(cp(p.accent)).cornerRadius(100.dp).clickable(openApp(ctx, "looty://app/tasks")), contentAlignment = Alignment.Center) {
                    Icon(R.drawable.ic_w_tasks, p.onAccent, 20.dp)
                }
                Spacer(GlanceModifier.width(10.dp))
                Column(GlanceModifier.defaultWeight().clickable(openApp(ctx, "looty://app/tasks"))) {
                    Text(ctx.getString(R.string.widget_title), maxLines = 1, style = TextStyle(color = cp(p.fg), fontSize = 16.sp, fontWeight = FontWeight.Bold))
                    if (!compact && data.tasks.isNotEmpty()) Text(ctx.resources.getQuantityString(R.plurals.widget_open_tasks, data.tasks.size, data.tasks.size), maxLines = 1, style = TextStyle(color = cp(p.soft), fontSize = 12.sp))
                }
                PillButton(R.drawable.ic_w_add, if (size.width > 200.dp) ctx.getString(R.string.widget_add) else null, p, openApp(ctx, "looty://app/quickadd"))
            }
            when {
                data.signedOut -> Message(ctx.getString(R.string.widget_sign_in), p, style, openApp(ctx))
                data.failed && data.tasks.isEmpty() -> Message(ctx.getString(R.string.widget_offline), p, style, openApp(ctx))
                data.tasks.isEmpty() -> Message(ctx.getString(R.string.widget_empty), p, style, openApp(ctx), R.drawable.ic_w_check)
                else -> Column(GlanceModifier.fillMaxWidth().defaultWeight()) {
                    // Pas d'espaceurs : Glance limite une colonne à 10 enfants.
                    data.tasks.take(fit).forEach { t -> TaskRow(ctx, data, t, p, style) }
                }
            }
        }
    }

    @Composable
    private fun TaskRow(ctx: Context, data: WidgetData, t: WidgetTask, p: Palette, style: WidgetStyle) {
        val late = t.due?.isBefore(LocalDate.now()) == true
        Box(GlanceModifier.fillMaxWidth().padding(bottom = 6.dp)) {
        Row(
            GlanceModifier.fillMaxWidth().height(40.dp).background(cp(p.chip)).cornerRadius(inner(style)).padding(start = 6.dp, end = 10.dp)
                .clickable(openApp(ctx, "looty://w/${data.workspace}/task/${t.id}")),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(GlanceModifier.size(32.dp).clickable(toggle(t.id)), contentAlignment = Alignment.Center) { Icon(R.drawable.ic_w_circle, p.accent, 22.dp) }
            Text(t.title, maxLines = 1, modifier = GlanceModifier.defaultWeight().padding(horizontal = 4.dp), style = TextStyle(color = cp(p.fg), fontSize = 14.sp, fontWeight = FontWeight.Medium))
            t.due?.let { d ->
                Spacer(GlanceModifier.width(6.dp))
                Text(dueLabel(ctx, d), maxLines = 1, style = TextStyle(color = cp(if (late) p.warn else p.soft), fontSize = 12.sp, fontWeight = if (late) FontWeight.Bold else FontWeight.Normal))
            }
        }
        }
    }
}

/** Message centré (vide, hors ligne, connexion), touche = ouvre l'application. */
@Composable
fun Message(text: String, p: Palette, style: WidgetStyle, action: androidx.glance.action.Action, icon: Int? = null) {
    Box(GlanceModifier.fillMaxSize().background(cp(p.chip)).cornerRadius(inner(style)).clickable(action), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            if (icon != null) { Icon(icon, p.accent, 26.dp); Spacer(GlanceModifier.height(4.dp)) }
            Text(text, style = TextStyle(color = cp(p.soft), fontSize = 14.sp), modifier = GlanceModifier.padding(horizontal = 12.dp))
        }
    }
}

fun dueLabel(ctx: Context, d: LocalDate): String {
    val today = LocalDate.now()
    return when (d) {
        today -> ctx.getString(R.string.today)
        today.plusDays(1) -> ctx.getString(R.string.tomorrow)
        else -> d.format(DateTimeFormatter.ofPattern("d MMM"))
    }
}

class LootyWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = LootyWidget()
    override fun onDeleted(context: Context, appWidgetIds: IntArray) { super.onDeleted(context, appWidgetIds); WidgetPrefs.forget(context, appWidgetIds) }
}
