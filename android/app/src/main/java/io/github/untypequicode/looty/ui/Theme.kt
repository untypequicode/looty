package io.github.untypequicode.looty.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val Indigo = Color(0xFF5E6AD2)

/** Thème choisi dans les réglages : « system », « light » ou « dark » (gardé sur l'appareil). */
object AppTheme {
    var mode by mutableStateOf<String?>(null)
    fun load(ctx: Context): String = mode ?: (ctx.getSharedPreferences("looty", 0).getString("theme", "system") ?: "system").also { mode = it }
    fun set(ctx: Context, m: String) { mode = m; ctx.getSharedPreferences("looty", 0).edit().putString("theme", m).apply() }
}

/** Material 3 Expressive : couleurs de l'appareil (Material You) sur Android 12+, sinon celles de Looty. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LootyTheme(content: @Composable () -> Unit) {
    val ctx = LocalContext.current
    val dark = when (AppTheme.load(ctx)) { "light" -> false; "dark" -> true; else -> isSystemInDarkTheme() }
    val scheme = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
        dark -> darkColorScheme(primary = Color(0xFF9EA5F0), onPrimary = Color(0xFF1C2270), primaryContainer = Color(0xFF353C9A), surface = Color(0xFF0F0F12), background = Color(0xFF0F0F12))
        else -> lightColorScheme(primary = Indigo, onPrimary = Color.White, primaryContainer = Color(0xFFE0E2FF), onPrimaryContainer = Color(0xFF14196B), surface = Color(0xFFFCFCFD), background = Color(0xFFFCFCFD))
    }
    // Icônes de la barre d'état et de navigation : lisibles sur le thème choisi (pas celui du téléphone).
    val view = androidx.compose.ui.platform.LocalView.current
    if (!view.isInEditMode) androidx.compose.runtime.SideEffect {
        (view.context as? android.app.Activity)?.window?.let { w ->
            androidx.core.view.WindowCompat.getInsetsController(w, view).apply { isAppearanceLightStatusBars = !dark; isAppearanceLightNavigationBars = !dark }
        }
    }
    MaterialExpressiveTheme(colorScheme = scheme, motionScheme = MotionScheme.expressive(), typography = Typography(), content = content)
}
