package io.github.untypequicode.looty.ui

import android.appwidget.AppWidgetManager
import android.content.ComponentName
import android.os.Build
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.widget.AgendaWidgetReceiver
import io.github.untypequicode.looty.widget.LootyWidgetReceiver
import io.github.untypequicode.looty.widget.ShortcutsWidgetReceiver
import io.github.untypequicode.looty.widget.TodayWidgetReceiver

private class WidgetEntry(val receiver: Class<*>, val label: Int, val hint: Int, val preview: Int)

private val ENTRIES = listOf(
    WidgetEntry(LootyWidgetReceiver::class.java, R.string.widget_title, R.string.widget_description, R.drawable.widget_preview),
    WidgetEntry(AgendaWidgetReceiver::class.java, R.string.widget_label_agenda, R.string.widget_agenda_description, R.drawable.widget_preview_agenda),
    WidgetEntry(TodayWidgetReceiver::class.java, R.string.widget_label_today, R.string.widget_today_description, R.drawable.widget_preview_today),
    WidgetEntry(ShortcutsWidgetReceiver::class.java, R.string.widget_label_shortcuts, R.string.widget_shortcuts_description, R.drawable.widget_preview_shortcuts),
)

/** Les widgets de Looty : aperçu et « Ajouter à l'écran d'accueil » (réglages à l'ajout). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WidgetsScreen(onBack: () -> Unit) {
    val ctx = LocalContext.current
    val manager = AppWidgetManager.getInstance(ctx)
    val canPin = Build.VERSION.SDK_INT >= 26 && manager.isRequestPinAppWidgetSupported
    Scaffold(
        topBar = { TopAppBar(title = { Text(stringResource(R.string.widgets_title)) }, navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, stringResource(R.string.back)) } }) },
    ) { padding ->
        LazyColumn(Modifier.padding(padding).fillMaxSize(), contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            item { Text(stringResource(R.string.widgets_hint), color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodyMedium) }
            items(ENTRIES) { w ->
                ElevatedCard(shape = RoundedCornerShape(28.dp)) {
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                        Image(
                            painterResource(w.preview), null, contentScale = ContentScale.Fit,
                            modifier = Modifier.fillMaxWidth().heightIn(max = 150.dp).clip(RoundedCornerShape(20.dp)),
                        )
                        Column {
                            Text(stringResource(w.label), style = MaterialTheme.typography.titleMedium)
                            Text(stringResource(w.hint), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        if (canPin) FilledTonalButton(
                            onClick = { manager.requestPinAppWidget(ComponentName(ctx, w.receiver), null, null) },
                            modifier = Modifier.align(Alignment.End),
                        ) { Text(stringResource(R.string.widgets_add)) }
                    }
                }
            }
            if (!canPin) item { Text(stringResource(R.string.widgets_manual), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
    }
}
