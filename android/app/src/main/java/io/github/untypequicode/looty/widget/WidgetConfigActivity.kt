package io.github.untypequicode.looty.widget

import android.app.Activity
import android.appwidget.AppWidgetManager
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.lifecycleScope
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.ui.LootyTheme
import kotlinx.coroutines.launch

/**
 * Réglages d'un widget : style (tonal, neutre, accent, contrasté), transparence, nombre
 * d'éléments, forme des coins et, pour le widget « Raccourcis », les boutons. Ouvert à
 * l'ajout du widget, puis par appui long sur le widget → Réglages.
 */
class WidgetConfigActivity : ComponentActivity() {
    @OptIn(ExperimentalMaterial3Api::class)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val id = intent?.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID) ?: AppWidgetManager.INVALID_APPWIDGET_ID
        // Fermé sans enregistrer : le widget n'est pas ajouté.
        setResult(Activity.RESULT_CANCELED, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id))
        if (id == AppWidgetManager.INVALID_APPWIDGET_ID) { finish(); return }
        val kind = AppWidgetManager.getInstance(this).getAppWidgetInfo(id)?.provider?.className.orEmpty()
        val isShortcuts = kind.endsWith("ShortcutsWidgetReceiver")
        val hasList = kind.endsWith("LootyWidgetReceiver") || kind.endsWith("AgendaWidgetReceiver")
        val initial = WidgetPrefs.load(this, id)

        setContent {
            LootyTheme {
                var style by remember { mutableStateOf(initial) }
                Scaffold(
                    topBar = {
                        TopAppBar(
                            title = { Text(stringResource(R.string.wc_title)) },
                            navigationIcon = { IconButton(onClick = { finish() }) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, stringResource(R.string.back)) } },
                        )
                    },
                    bottomBar = {
                        Surface(tonalElevation = 3.dp) {
                            Button(
                                onClick = {
                                    WidgetPrefs.save(this@WidgetConfigActivity, id, style)
                                    lifecycleScope.launch {
                                        Widgets.updateAll(applicationContext)
                                        setResult(Activity.RESULT_OK, Intent().putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id))
                                        finish()
                                    }
                                },
                                modifier = Modifier.fillMaxWidth().navigationBarsPadding().padding(16.dp).height(52.dp),
                            ) { Text(stringResource(R.string.wc_save)) }
                        }
                    },
                ) { padding ->
                    Column(Modifier.padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(20.dp)) {
                        Preview(style)
                        Section(stringResource(R.string.wc_look)) {
                            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                                WidgetStyle.LOOKS.forEach { l -> LookTile(l, selected = style.look == l, modifier = Modifier.weight(1f)) { style = style.copy(look = l) } }
                            }
                        }
                        Section(stringResource(R.string.wc_opacity, style.opacity)) {
                            Slider(value = style.opacity.toFloat(), onValueChange = { style = style.copy(opacity = (it / 5).toInt() * 5) }, valueRange = 30f..100f)
                        }
                        if (hasList) Section(stringResource(R.string.wc_items, style.items)) {
                            Slider(value = style.items.toFloat(), onValueChange = { style = style.copy(items = it.toInt()) }, valueRange = 1f..9f, steps = 7)
                        }
                        ListItem(
                            headlineContent = { Text(stringResource(R.string.wc_round)) },
                            supportingContent = { Text(stringResource(R.string.wc_round_hint)) },
                            trailingContent = { Switch(checked = style.round, onCheckedChange = { style = style.copy(round = it) }) },
                            modifier = Modifier.clickable { style = style.copy(round = !style.round) },
                            colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                        )
                        if (isShortcuts) Section(stringResource(R.string.wc_shortcuts, style.shortcuts.size, WidgetStyle.MAX_SHORTCUTS)) {
                            FlowRowChips(style.shortcuts) { style = style.copy(shortcuts = it) }
                        }
                        Spacer(Modifier.height(8.dp))
                    }
                }
            }
        }
    }
}

@Composable
private fun Section(title: String, content: @Composable ColumnScope.() -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(title, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
        content()
    }
}

/** Couleurs de fond / texte d'un style, pour l'aperçu et les pastilles de choix. */
@Composable
private fun swatch(look: String): Pair<Color, Color> {
    val c = MaterialTheme.colorScheme
    return when (look) {
        "surface" -> c.surface to c.onSurface
        "accent" -> c.primary to c.onPrimary
        "contrast" -> c.inverseSurface to c.inverseOnSurface
        else -> c.secondaryContainer to c.onSecondaryContainer
    }
}

@Composable
private fun LookTile(look: String, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val (bg, fg) = swatch(look)
    val label = when (look) { "surface" -> R.string.wc_look_surface; "accent" -> R.string.wc_look_accent; "contrast" -> R.string.wc_look_contrast; else -> R.string.wc_look_tonal }
    Column(
        modifier.clip28().background(bg).border(BorderStroke(if (selected) 3.dp else 1.dp, if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant), RoundedCornerShape(24.dp))
            .clickable(onClick = onClick).padding(vertical = 14.dp),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(Modifier.size(22.dp).background(fg, RoundedCornerShape(50)))
        Text(stringResource(label), color = fg, style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.Medium)
    }
}

private fun Modifier.clip28() = this.clip(RoundedCornerShape(24.dp))

/** Aperçu approximatif : fond (avec transparence), coins, trois lignes d'exemple. */
@Composable
private fun Preview(style: WidgetStyle) {
    val (bg, fg) = swatch(style.look)
    val radius = if (style.round) 36.dp else 22.dp
    Box(Modifier.fillMaxWidth().padding(top = 8.dp).background(MaterialTheme.colorScheme.surfaceVariant, RoundedCornerShape(28.dp)).padding(20.dp)) {
        Column(
            Modifier.fillMaxWidth().background(bg.copy(alpha = style.opacity / 100f), RoundedCornerShape(radius)).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(Modifier.size(34.dp).background(MaterialTheme.colorScheme.primary.takeIf { style.look != "accent" } ?: fg, RoundedCornerShape(50)))
                Text(stringResource(R.string.widget_title), color = fg, fontWeight = FontWeight.Bold, style = MaterialTheme.typography.titleMedium)
            }
            repeat(minOf(style.items, 3)) { i ->
                Row(
                    Modifier.fillMaxWidth().background(fg.copy(alpha = .12f), RoundedCornerShape(if (style.round) 22.dp else 14.dp)).padding(horizontal = 12.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Box(Modifier.size(18.dp).border(2.dp, fg, RoundedCornerShape(50)))
                    Box(Modifier.height(10.dp).weight(1f).alpha(.75f).background(fg, RoundedCornerShape(50)))
                    Box(Modifier.height(10.dp).width(36.dp + (i * 6).dp).alpha(.4f).background(fg, RoundedCornerShape(50)))
                }
            }
        }
    }
}

/** Choix des boutons du widget « Raccourcis » : touche pour ajouter ou retirer, dans l'ordre. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FlowRowChips(selected: List<String>, onChange: (List<String>) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        WidgetStyle.SHORTCUTS.forEach { key ->
            val s = SHORTCUTS[key] ?: return@forEach
            val on = key in selected
            FilterChip(
                selected = on,
                onClick = {
                    val next = if (on) selected - key else if (selected.size < WidgetStyle.MAX_SHORTCUTS) selected + key else selected
                    if (next.isNotEmpty()) onChange(next)
                },
                label = { Text(if (on) "${selected.indexOf(key) + 1} · ${stringResource(s.label)}" else stringResource(s.label)) },
            )
        }
    }
}
