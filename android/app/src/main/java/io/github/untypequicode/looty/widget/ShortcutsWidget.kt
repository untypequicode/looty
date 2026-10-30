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
import androidx.glance.layout.fillMaxHeight
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import io.github.untypequicode.looty.R

/** Un raccourci : icône, libellé et lien ouvert dans l'application. */
class Shortcut(val icon: Int, val label: Int, val link: String)

val SHORTCUTS: Map<String, Shortcut> = mapOf(
    "task" to Shortcut(R.drawable.ic_w_add, R.string.widget_sc_task, "looty://app/quickadd"),
    "inbox" to Shortcut(R.drawable.ic_w_bell, R.string.widget_sc_inbox, "looty://app/inbox"),
    "calendar" to Shortcut(R.drawable.ic_w_calendar, R.string.widget_sc_calendar, "looty://app/calendar"),
    "docs" to Shortcut(R.drawable.ic_w_doc, R.string.widget_sc_docs, "looty://app/docs"),
    "tasks" to Shortcut(R.drawable.ic_w_tasks, R.string.widget_sc_tasks, "looty://app/tasks"),
    "mail" to Shortcut(R.drawable.ic_w_mail, R.string.widget_sc_mail, "looty://app/mail"),
    "moodle" to Shortcut(R.drawable.ic_w_school, R.string.widget_sc_moodle, "looty://app/moodle"),
    "drive" to Shortcut(R.drawable.ic_w_folder, R.string.widget_sc_drive, "looty://app/drive"),
    "projects" to Shortcut(R.drawable.ic_w_doc, R.string.widget_sc_projects, "looty://app/projects"),
)

/**
 * Widget « Raccourcis » : de gros boutons arrondis (1 à 5 au choix) vers l'ajout rapide et les
 * écrans les plus utilisés. Le premier est mis en avant avec la couleur d'accent.
 */
class ShortcutsWidget : GlanceAppWidget() {
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val style = Widgets.styleOf(context, id)
        provideContent { GlanceTheme { Content(context, style) } }
    }

    @Composable
    private fun Content(ctx: Context, style: WidgetStyle) {
        val p = palette(style)
        val size = LocalSize.current
        val labels = size.height >= 96.dp
        val items = style.shortcuts.mapNotNull { SHORTCUTS[it] }.take(if (size.width < 160.dp) 3 else WidgetStyle.MAX_SHORTCUTS)
        Row(GlanceModifier.fillMaxSize().background(cp(p.bg)).cornerRadius(outer(style)).padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            items.forEachIndexed { i, s ->
                if (i > 0) Spacer(GlanceModifier.width(6.dp))
                val first = i == 0
                Column(
                    GlanceModifier.defaultWeight().fillMaxHeight().background(cp(if (first) p.accent else p.chip)).cornerRadius(inner(style) + 4.dp).clickable(openApp(ctx, s.link)),
                    verticalAlignment = Alignment.CenterVertically, horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(s.icon, if (first) p.onAccent else p.accent, 26.dp)
                    if (labels) {
                        Spacer(GlanceModifier.height(4.dp))
                        Text(ctx.getString(s.label), maxLines = 1, style = TextStyle(color = cp(if (first) p.onAccent else p.fg), fontSize = 12.sp, fontWeight = FontWeight.Medium))
                    }
                }
            }
        }
    }
}

class ShortcutsWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = ShortcutsWidget()
    override fun onDeleted(context: Context, appWidgetIds: IntArray) { super.onDeleted(context, appWidgetIds); WidgetPrefs.forget(context, appWidgetIds) }
}
