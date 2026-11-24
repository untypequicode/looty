package io.github.untypequicode.looty.update

import io.github.untypequicode.looty.BuildConfig

/**
 * Variante « fdroid » : aucune mise à jour intégrée (F-Droid interdit qu'une application installe
 * ses propres mises à jour). Seule la version installée est connue ; la variante « direct » a le vrai code.
 */
object Updater {
    /** Version installée, sans le suffixe des variantes de test (« -debug », « -qa »). */
    val current: String get() = BuildConfig.VERSION_NAME.substringBefore('-')
}
