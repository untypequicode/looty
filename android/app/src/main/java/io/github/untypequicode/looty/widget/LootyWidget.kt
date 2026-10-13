package io.github.untypequicode.looty.widget

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.cornerRadius
import androidx.glance.LocalSize
import androidx.glance.appwidget.SizeMode
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Column
import androidx.glance.layout.Row
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.padding
import androidx.glance.layout.width
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import io.github.untypequicode.looty.MainActivity
import io.github.untypequicode.looty.R
import java.time.LocalDate
import java.time.format.DateTimeFormatter

/**
 * Widget d'écran d'accueil : mes tâches à faire. Toucher une tâche l'ouvre dans l'application
 * (écran natif), l'en-tête ouvre l'application. Mis à jour toutes les 30 minutes et à chaque
 * rafraîchissement de l'application.
 */
class LootyWidget : GlanceAppWidget() {
    // Taille réelle du widget : on affiche autant de tâches qu’il peut en contenir.
    override val sizeMode = SizeMode.Exact

    override suspend fun provideGlance(context: Context, id: GlanceId) {
        val data = WidgetData.load(context)
        provideContent { GlanceTheme { Content(context, data) } }
    }

    @Composable
    private fun Content(ctx: Context, data: WidgetData) {
        val open = actionStartActivity(Intent(ctx, MainActivity::class.java))
        Column(GlanceModifier.fillMaxSize().background(GlanceTheme.colors.widgetBackground).cornerRadius(24.dp).padding(12.dp)) {
            Row(GlanceModifier.fillMaxWidth().padding(bottom = 6.dp).clickable(open), verticalAlignment = Alignment.CenterVertically) {
                Text(ctx.getString(R.string.widget_title), style = TextStyle(color = GlanceTheme.colors.primary, fontSize = 15.sp, fontWeight = FontWeight.Bold))
                Spacer(GlanceModifier.defaultWeight())
                if (data.tasks.isNotEmpty()) Text(data.tasks.size.toString(), style = TextStyle(color = GlanceTheme.colors.onSurfaceVariant, fontSize = 13.sp))
            }
            when {
                data.signedOut -> Text(ctx.getString(R.string.widget_sign_in), modifier = GlanceModifier.clickable(open), style = TextStyle(color = GlanceTheme.colors.onSurfaceVariant, fontSize = 14.sp))
                data.failed -> Text(ctx.getString(R.string.widget_offline), modifier = GlanceModifier.clickable(open), style = TextStyle(color = GlanceTheme.colors.onSurfaceVariant, fontSize = 14.sp))
                data.tasks.isEmpty() -> Text(ctx.getString(R.string.widget_empty), modifier = GlanceModifier.clickable(open), style = TextStyle(color = GlanceTheme.colors.onSurfaceVariant, fontSize = 14.sp))
                else -> Column(GlanceModifier.fillMaxWidth().defaultWeight()) {
                    val fit = ((LocalSize.current.height - 56.dp) / 34.dp).toInt().coerceIn(1, 12)
                    data.tasks.take(fit).forEach { t ->
                        val intent = Intent(Intent.ACTION_VIEW, Uri.parse("looty://w/${data.workspace}/task/${t.id}")).setPackage(ctx.packageName)
                        Row(GlanceModifier.fillMaxWidth().padding(vertical = 6.dp).clickable(actionStartActivity(intent)), verticalAlignment = Alignment.CenterVertically) {
                            Text(t.title, maxLines = 1, modifier = GlanceModifier.defaultWeight(), style = TextStyle(color = GlanceTheme.colors.onSurface, fontSize = 14.sp))
                            t.due?.let { d ->
                                Spacer(GlanceModifier.width(8.dp))
                                Text(dueLabel(ctx, d), style = TextStyle(color = if (d.isBefore(LocalDate.now())) GlanceTheme.colors.error else GlanceTheme.colors.onSurfaceVariant, fontSize = 12.sp))
                            }
                        }
                    }
                }
            }
        }
    }

    private fun dueLabel(ctx: Context, d: LocalDate): String {
        val today = LocalDate.now()
        return when (d) {
            today -> ctx.getString(R.string.today)
            today.plusDays(1) -> ctx.getString(R.string.tomorrow)
            else -> d.format(DateTimeFormatter.ofPattern("d MMM"))
        }
    }
}

class LootyWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = LootyWidget()
}
