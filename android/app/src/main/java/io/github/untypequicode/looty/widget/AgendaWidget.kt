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
import androidx.glance.text.TextDecoration
import androidx.glance.text.TextStyle
import io.github.untypequicode.looty.R
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.TextStyle as JTextStyle
import java.util.Locale

/** Widget « Agenda » : événements et échéances des prochains jours, groupés par jour. */
class AgendaWidget : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val data = WidgetAgenda.load(context)
        val style = Widgets.styleOf(context, id)
        provideContent { GlanceTheme { Content(context, data, style) } }
    }

    @Composable
    private fun Content(ctx: Context, data: WidgetAgenda, style: WidgetStyle) {
        val p = palette(style)
        val size = LocalSize.current
        val compact = size.height < 140.dp
        // Lignes (jours et éléments) qui tiennent dans la hauteur, dans la limite choisie.
        val room = size.height - (if (compact) 60.dp else 74.dp)
        val days = mutableListOf<Pair<LocalDate, MutableList<AgendaItem>>>()
        var used = 0.dp
        var shown = 0
        for (it in data.items) {
            val head = if (days.lastOrNull()?.first != it.day) 26.dp else 0.dp
            if (used + head + 38.dp > room || shown >= style.items) break
            if (head > 0.dp) days += it.day to mutableListOf()
            days.last().second += it
            used += head + 38.dp
            shown++
        }
        Column(GlanceModifier.fillMaxSize().background(cp(p.bg)).cornerRadius(outer(style)).padding(14.dp)) {
            Row(GlanceModifier.fillMaxWidth().padding(bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                Box(GlanceModifier.size(34.dp).background(cp(p.accent)).cornerRadius(100.dp).clickable(openApp(ctx, "looty://app/calendar")), contentAlignment = Alignment.Center) {
                    Icon(R.drawable.ic_w_calendar, p.onAccent, 19.dp)
                }
                Spacer(GlanceModifier.width(10.dp))
                Column(GlanceModifier.defaultWeight().clickable(openApp(ctx, "looty://app/calendar"))) {
                    Text(ctx.getString(R.string.widget_agenda_title), maxLines = 1, style = TextStyle(color = cp(p.fg), fontSize = 16.sp, fontWeight = FontWeight.Bold))
                    if (!compact) Text(LocalDate.now().format(DateTimeFormatter.ofPattern("EEEE d MMMM")), maxLines = 1, style = TextStyle(color = cp(p.soft), fontSize = 12.sp))
                }
                PillButton(R.drawable.ic_w_add, null, p, openApp(ctx, "looty://app/quickadd"))
            }
            when {
                data.signedOut -> Message(ctx.getString(R.string.widget_sign_in), p, style, openApp(ctx))
                data.failed && data.items.isEmpty() -> Message(ctx.getString(R.string.widget_offline), p, style, openApp(ctx))
                data.items.isEmpty() -> Message(ctx.getString(R.string.widget_agenda_empty), p, style, openApp(ctx), R.drawable.ic_w_calendar)
                else -> Column(GlanceModifier.fillMaxWidth().defaultWeight()) {
                    // Un groupe par jour (une colonne par jour : Glance limite les enfants d'une colonne).
                    days.forEach { (day, rows) ->
                        Column(GlanceModifier.fillMaxWidth()) {
                            Text(dayLabel(ctx, day), maxLines = 1, modifier = GlanceModifier.padding(top = 6.dp, bottom = 4.dp, start = 4.dp), style = TextStyle(color = cp(if (day == LocalDate.now()) p.accent else p.soft), fontSize = 12.sp, fontWeight = FontWeight.Bold))
                            rows.forEach { Item(ctx, it, p, style) }
                        }
                    }
                }
            }
        }
    }

    @Composable
    private fun Item(ctx: Context, a: AgendaItem, p: Palette, style: WidgetStyle) {
        val late = a.isTask && !a.done && a.day.isBefore(LocalDate.now())
        Box(GlanceModifier.fillMaxWidth().padding(bottom = 4.dp)) {
        Row(
            GlanceModifier.fillMaxWidth().height(34.dp).background(cp(p.chip)).cornerRadius(inner(style)).clickable(openApp(ctx, "looty:/" + a.route)),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // Barre de couleur : événement (couleur d'accent) ou échéance (case à cocher).
            if (a.isTask && a.taskId != null) {
                Box(GlanceModifier.size(34.dp).clickable(toggle(a.taskId)), contentAlignment = Alignment.Center) { Icon(if (a.done) R.drawable.ic_w_tasks else R.drawable.ic_w_circle, if (late) p.warn else p.accent, 20.dp) }
            } else {
                Spacer(GlanceModifier.width(8.dp))
                Box(GlanceModifier.width(4.dp).height(18.dp).background(cp(p.accent)).cornerRadius(100.dp)) {}
                Spacer(GlanceModifier.width(6.dp))
            }
            Text(a.title, maxLines = 1, modifier = GlanceModifier.defaultWeight().padding(horizontal = 4.dp), style = TextStyle(color = cp(if (a.done) p.soft else p.fg), fontSize = 13.sp, fontWeight = FontWeight.Medium, textDecoration = if (a.done) TextDecoration.LineThrough else TextDecoration.None))
            Text(a.time?.format(DateTimeFormatter.ofPattern("HH:mm")) ?: if (a.isTask) ctx.getString(R.string.widget_deadline) else ctx.getString(R.string.widget_all_day), maxLines = 1, modifier = GlanceModifier.padding(end = 10.dp), style = TextStyle(color = cp(if (late) p.warn else p.soft), fontSize = 12.sp))
        }
        }
    }

    private fun dayLabel(ctx: Context, d: LocalDate): String {
        val today = LocalDate.now()
        return when (d) {
            today -> ctx.getString(R.string.today)
            today.plusDays(1) -> ctx.getString(R.string.tomorrow)
            else -> d.dayOfWeek.getDisplayName(JTextStyle.FULL, Locale.getDefault()).replaceFirstChar { it.uppercase() } + " " + d.dayOfMonth
        }
    }
}

class AgendaWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = AgendaWidget()
    override fun onDeleted(context: Context, appWidgetIds: IntArray) { super.onDeleted(context, appWidgetIds); WidgetPrefs.forget(context, appWidgetIds) }
}
