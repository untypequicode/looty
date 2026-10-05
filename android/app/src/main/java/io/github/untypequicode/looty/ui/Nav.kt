package io.github.untypequicode.looty.ui

import androidx.compose.runtime.staticCompositionLocalOf

/** Ouverture des écrans natifs de détail depuis n'importe quel écran. */
class Opener(val task: (String) -> Unit, val board: (String) -> Unit)

val LocalOpener = staticCompositionLocalOf { Opener({}, {}) }
