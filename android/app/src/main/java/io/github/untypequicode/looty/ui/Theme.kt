package io.github.untypequicode.looty.ui

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val Indigo = Color(0xFF5E6AD2)

/** Material 3 Expressive : couleurs de l'appareil (Material You) sur Android 12+, sinon celles de Looty. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun LootyTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val ctx = LocalContext.current
    val scheme = when {
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
        dark -> darkColorScheme(primary = Color(0xFF9EA5F0), onPrimary = Color(0xFF1C2270), primaryContainer = Color(0xFF353C9A), surface = Color(0xFF0F0F12), background = Color(0xFF0F0F12))
        else -> lightColorScheme(primary = Indigo, onPrimary = Color.White, primaryContainer = Color(0xFFE0E2FF), onPrimaryContainer = Color(0xFF14196B), surface = Color(0xFFFCFCFD), background = Color(0xFFFCFCFD))
    }
    MaterialExpressiveTheme(colorScheme = scheme, motionScheme = MotionScheme.expressive(), typography = Typography(), content = content)
}
