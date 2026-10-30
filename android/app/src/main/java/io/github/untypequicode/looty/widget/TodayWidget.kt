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
import java.time.format.DateTimeFormatter

/**
 * Widget « Aujourd'hui » (petit) : le nombre de tâches à faire aujourd'hui ou en retard, et le
 * prochain événement. Un grand chiffre, des formes pleines : lisible d'un coup d'œil.
 */
class TodayWidget : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val data = WidgetAgenda.load(context)
        // Tâches à faire (échéances en retard comprises) : l'agenda ne remonte que les prochains jours.
        val tasks = WidgetData.load(context)
        val style = Widgets.styleOf(context, id)
        provideContent { GlanceTheme { Content(context, data, tasks, style) } }
    }

    @Composable
    private fun Content(ctx: Context, data: WidgetAgenda, tasks: WidgetData, style: WidgetStyle) {
        val p = palette(style)
        val size = LocalSize.current
        val wide = size.width >= 200.dp
        val today = java.time.LocalDate.now()
        val n = tasks.tasks.count { it.due != null && !it.due.isAfter(today) }
        val overdue = tasks.tasks.count { it.due != null && it.due.isBefore(today) }
        val next = data.nextEvent()
        Column(GlanceModifier.fillMaxSize().background(cp(p.bg)).cornerRadius(outer(style)).padding(14.dp).clickable(openApp(ctx, "looty://app/tasks"))) {
            Row(GlanceModifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Text(ctx.getString(R.string.today), maxLines = 1, modifier = GlanceModifier.defaultWeight(), style = TextStyle(color = cp(p.soft), fontSize = 14.sp, fontWeight = FontWeight.Medium))
                PillButton(R.drawable.ic_w_add, null, p, openApp(ctx, "looty://app/quickadd"))
            }
            when {
                data.signedOut -> Text(ctx.getString(R.string.widget_sign_in), style = TextStyle(color = cp(p.fg), fontSize = 14.sp), modifier = GlanceModifier.padding(top = 8.dp))
                data.failed && data.items.isEmpty() -> Text(ctx.getString(R.string.widget_offline), style = TextStyle(color = cp(p.fg), fontSize = 14.sp), modifier = GlanceModifier.padding(top = 8.dp))
                else -> {
                    Spacer(GlanceModifier.defaultWeight())
                    Row(verticalAlignment = Alignment.Bottom) {
                        Text(n.toString(), style = TextStyle(color = cp(if (overdue > 0) p.warn else p.fg), fontSize = if (wide) 56.sp else 48.sp, fontWeight = FontWeight.Bold))
                        Spacer(GlanceModifier.width(8.dp))
                        Column(GlanceModifier.padding(bottom = 8.dp)) {
                            Text(ctx.resources.getQuantityString(R.plurals.widget_tasks_today, n), maxLines = 1, style = TextStyle(color = cp(p.fg), fontSize = 14.sp, fontWeight = FontWeight.Medium))
                            if (overdue > 0) Text(ctx.resources.getQuantityString(R.plurals.widget_overdue, overdue, overdue), maxLines = 1, style = TextStyle(color = cp(p.warn), fontSize = 12.sp, fontWeight = FontWeight.Bold))
                        }
                    }
                    Spacer(GlanceModifier.height(8.dp))
                    Row(GlanceModifier.fillMaxWidth().background(cp(p.chip)).cornerRadius(inner(style)).padding(horizontal = 10.dp, vertical = 7.dp).clickable(openApp(ctx, "looty://app/calendar")), verticalAlignment = Alignment.CenterVertically) {
                        Icon(R.drawable.ic_w_calendar, p.accent, 16.dp)
                        Spacer(GlanceModifier.width(6.dp))
                        Text(
                            next?.let { (it.time?.format(DateTimeFormatter.ofPattern("HH:mm"))?.plus(" · ") ?: "") + it.title } ?: ctx.getString(R.string.widget_no_event),
                            maxLines = 1, style = TextStyle(color = cp(p.fg), fontSize = 13.sp, fontWeight = FontWeight.Medium),
                        )
                    }
                }
            }
        }
    }
}

class TodayWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = TodayWidget()
    override fun onDeleted(context: Context, appWidgetIds: IntArray) { super.onDeleted(context, appWidgetIds); WidgetPrefs.forget(context, appWidgetIds) }
}
