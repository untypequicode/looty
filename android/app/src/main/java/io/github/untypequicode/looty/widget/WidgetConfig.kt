package io.github.untypequicode.looty.widget

import android.content.Context

/** Apparence choisie pour un widget (réglable à tout moment : appui long sur le widget → Réglages). */
data class WidgetStyle(
    /** tonal (couleur du thème, douce), surface (neutre), accent (couleur forte), contrast (sombre/clair franc). */
    val look: String = "tonal",
    /** Opacité du fond, 30 à 100 %. */
    val opacity: Int = 100,
    /** Nombre maximal d'éléments listés (selon la place disponible). */
    val items: Int = 6,
    /** Coins : arrondis très marqués (Material 3 Expressive) ou plus sobres. */
    val round: Boolean = true,
    /** Boutons de raccourcis affichés (widget « Raccourcis »), dans l'ordre. */
    val shortcuts: List<String> = listOf("task", "inbox", "calendar", "docs"),
) {
    companion object {
        val LOOKS = listOf("tonal", "surface", "accent", "contrast")
        val SHORTCUTS = listOf("task", "inbox", "calendar", "docs", "tasks", "mail", "moodle", "drive", "projects")
        const val MAX_SHORTCUTS = 5
        /** Glance limite une colonne à 10 enfants : au plus 9 éléments listés. */
        const val MAX_ITEMS = 9
    }
}

/** Réglages par widget (identifiant Android du widget), gardés sur l'appareil. */
object WidgetPrefs {
    private fun prefs(ctx: Context) = ctx.getSharedPreferences("looty.widget.cfg", 0)

    fun load(ctx: Context, id: Int): WidgetStyle {
        val p = prefs(ctx)
        val d = WidgetStyle()
        return WidgetStyle(
            look = p.getString("$id.look", d.look)?.takeIf { it in WidgetStyle.LOOKS } ?: d.look,
            opacity = p.getInt("$id.opacity", d.opacity).coerceIn(30, 100),
            items = p.getInt("$id.items", d.items).coerceIn(1, WidgetStyle.MAX_ITEMS),
            round = p.getBoolean("$id.round", d.round),
            shortcuts = p.getString("$id.shortcuts", null)?.split(',')?.filter { it in WidgetStyle.SHORTCUTS }?.take(WidgetStyle.MAX_SHORTCUTS)?.takeIf { it.isNotEmpty() } ?: d.shortcuts,
        )
    }

    fun save(ctx: Context, id: Int, s: WidgetStyle) {
        prefs(ctx).edit()
            .putString("$id.look", s.look).putInt("$id.opacity", s.opacity).putInt("$id.items", s.items)
            .putBoolean("$id.round", s.round).putString("$id.shortcuts", s.shortcuts.joinToString(","))
            .apply()
    }

    fun forget(ctx: Context, ids: IntArray) {
        val e = prefs(ctx).edit()
        val keys = prefs(ctx).all.keys
        ids.forEach { id -> keys.filter { it.startsWith("$id.") }.forEach(e::remove) }
        e.apply()
    }
}
