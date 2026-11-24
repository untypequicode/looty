package io.github.untypequicode.looty.push

import android.content.Context
import io.github.untypequicode.looty.InstanceStore
import io.github.untypequicode.looty.Instances

/** Mode de réception des notifications sur cet appareil. */
enum class PushMode { OFF, UNIFIED_PUSH, POLLING }

/**
 * Réglages des notifications, propres à l'appareil et à chaque instance ([inst] : instance active
 * par défaut ; le service UnifiedPush et la tâche de fond passent l'instance concernée).
 */
object PushStore {
    private fun prefs(ctx: Context, inst: String) = ctx.getSharedPreferences(InstanceStore.scoped("looty.push", inst), Context.MODE_PRIVATE)

    fun mode(ctx: Context, inst: String = Instances.activeId(ctx)): PushMode =
        runCatching { PushMode.valueOf(prefs(ctx, inst).getString("mode", PushMode.OFF.name)!!) }.getOrDefault(PushMode.OFF)
    fun setMode(ctx: Context, m: PushMode, inst: String = Instances.activeId(ctx)) = prefs(ctx, inst).edit().putString("mode", m.name).apply()

    /** Abonnement enregistré sur le serveur pour cet appareil (pour le supprimer). */
    fun subscriptionId(ctx: Context, inst: String = Instances.activeId(ctx)): String? = prefs(ctx, inst).getString("subId", null)
    fun setSubscriptionId(ctx: Context, id: String?, inst: String = Instances.activeId(ctx)) =
        prefs(ctx, inst).edit().apply { if (id == null) remove("subId") else putString("subId", id) }.apply()

    /** Dernière erreur d'enregistrement, affichée dans les réglages. */
    fun error(ctx: Context, inst: String = Instances.activeId(ctx)): String? = prefs(ctx, inst).getString("error", null)
    fun setError(ctx: Context, e: String?, inst: String = Instances.activeId(ctx)) =
        prefs(ctx, inst).edit().apply { if (e == null) remove("error") else putString("error", e) }.apply()

    /** Vérification périodique : horodatage (ms) de la dernière vérification et notifications déjà montrées. */
    fun lastPoll(ctx: Context, inst: String = Instances.activeId(ctx)): Long = prefs(ctx, inst).getLong("lastPoll", 0L)
    fun setLastPoll(ctx: Context, t: Long, inst: String = Instances.activeId(ctx)) = prefs(ctx, inst).edit().putLong("lastPoll", t).apply()
    fun shown(ctx: Context, inst: String = Instances.activeId(ctx)): Set<String> = prefs(ctx, inst).getStringSet("shown", emptySet()) ?: emptySet()
    fun setShown(ctx: Context, ids: Set<String>, inst: String = Instances.activeId(ctx)) = prefs(ctx, inst).edit().putStringSet("shown", ids).apply()

    /** Efface tous les réglages de notifications d'une instance retirée. */
    fun clear(ctx: Context, inst: String) = prefs(ctx, inst).edit().clear().apply()
}
