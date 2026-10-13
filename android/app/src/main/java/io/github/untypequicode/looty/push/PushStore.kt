package io.github.untypequicode.looty.push

import android.content.Context

/** Mode de réception des notifications sur cet appareil. */
enum class PushMode { OFF, UNIFIED_PUSH, POLLING }

/** Réglages des notifications (propres à l'appareil). */
object PushStore {
    private fun prefs(ctx: Context) = ctx.getSharedPreferences("looty.push", Context.MODE_PRIVATE)

    fun mode(ctx: Context): PushMode = runCatching { PushMode.valueOf(prefs(ctx).getString("mode", PushMode.OFF.name)!!) }.getOrDefault(PushMode.OFF)
    fun setMode(ctx: Context, m: PushMode) = prefs(ctx).edit().putString("mode", m.name).apply()

    /** Abonnement enregistré sur le serveur pour cet appareil (pour le supprimer). */
    fun subscriptionId(ctx: Context): String? = prefs(ctx).getString("subId", null)
    fun setSubscriptionId(ctx: Context, id: String?) = prefs(ctx).edit().apply { if (id == null) remove("subId") else putString("subId", id) }.apply()

    /** Dernière erreur d'enregistrement, affichée dans les réglages. */
    fun error(ctx: Context): String? = prefs(ctx).getString("error", null)
    fun setError(ctx: Context, e: String?) = prefs(ctx).edit().apply { if (e == null) remove("error") else putString("error", e) }.apply()

    /** Vérification périodique : horodatage (ms) de la dernière vérification et notifications déjà montrées. */
    fun lastPoll(ctx: Context): Long = prefs(ctx).getLong("lastPoll", 0L)
    fun setLastPoll(ctx: Context, t: Long) = prefs(ctx).edit().putLong("lastPoll", t).apply()
    fun shown(ctx: Context): Set<String> = prefs(ctx).getStringSet("shown", emptySet()) ?: emptySet()
    fun setShown(ctx: Context, ids: Set<String>) = prefs(ctx).edit().putStringSet("shown", ids).apply()
}
