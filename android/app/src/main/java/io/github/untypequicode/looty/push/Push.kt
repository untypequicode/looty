package io.github.untypequicode.looty.push

import android.app.Activity
import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import io.github.untypequicode.looty.api.Api
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.unifiedpush.android.connector.UnifiedPush
import java.util.concurrent.TimeUnit

/**
 * Notifications sur cet appareil, sans service Google :
 *  - UnifiedPush (ntfy, Gotify, NextPush…) : le serveur Looty envoie un message chiffré
 *    (Web Push) à l'adresse fournie par l'application de distribution ;
 *  - à défaut, vérification périodique des notifications (toutes les ~15 minutes).
 */
object Push {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private const val POLL_WORK = "looty-poll"

    /** Applications de distribution UnifiedPush installées. */
    fun distributors(ctx: Context): List<String> = runCatching { UnifiedPush.getDistributors(ctx) }.getOrDefault(emptyList())

    fun distributorName(ctx: Context): String? {
        val pkg = runCatching { UnifiedPush.getAckDistributor(ctx) ?: UnifiedPush.getSavedDistributor(ctx) }.getOrNull() ?: return null
        return runCatching { ctx.packageManager.getApplicationLabel(ctx.packageManager.getApplicationInfo(pkg, 0)).toString() }.getOrDefault(pkg)
    }

    /**
     * Active UnifiedPush : choix de l'application de distribution si besoin (fenêtre système),
     * puis enregistrement. `done(true)` = demande envoyée ; l'adresse arrive ensuite au service.
     */
    fun enableUnifiedPush(activity: Activity, done: (Boolean) -> Unit) {
        PushStore.setError(activity, null)
        UnifiedPush.tryUseCurrentOrDefaultDistributor(activity) { ok ->
            if (!ok) return@tryUseCurrentOrDefaultDistributor done(false)
            registerNow(activity.applicationContext, done)
        }
    }

    /** Autre application de distribution. */
    fun pickDistributor(activity: Activity, done: (Boolean) -> Unit) {
        UnifiedPush.tryPickDistributor(activity) { ok ->
            if (!ok) return@tryPickDistributor done(false)
            registerNow(activity.applicationContext, done)
        }
    }

    /** Enregistrement auprès du distributeur, limité à la clé VAPID de ce serveur. */
    private fun registerNow(ctx: Context, done: (Boolean) -> Unit) {
        scope.launch {
            val vapid = runCatching { Api(ctx).get<JsonObject>("/api/user/push/key")["key"]?.jsonPrimitive?.content }.getOrNull()
            if (vapid == null) {
                PushStore.setError(ctx, "server")
                return@launch kotlinx.coroutines.withContext(Dispatchers.Main) { done(false) }
            }
            val ok = runCatching { UnifiedPush.register(ctx, messageForDistributor = "Looty", vapid = vapid) }.isSuccess
            if (ok) {
                PushStore.setMode(ctx, PushMode.UNIFIED_PUSH)
                stopPolling(ctx)
            } else PushStore.setError(ctx, "vapid")
            kotlinx.coroutines.withContext(Dispatchers.Main) { done(ok) }
        }
    }

    /** Vérification périodique (plus tôt que 15 minutes n'est pas permis par Android). */
    fun enablePolling(ctx: Context) {
        runCatching { UnifiedPush.unregister(ctx) }
        removeSubscription(ctx)
        PushStore.setMode(ctx, PushMode.POLLING)
        PushStore.setError(ctx, null)
        PushStore.setLastPoll(ctx, System.currentTimeMillis())
        val req = PeriodicWorkRequestBuilder<PollWorker>(15, TimeUnit.MINUTES)
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .build()
        WorkManager.getInstance(ctx).enqueueUniquePeriodicWork(POLL_WORK, ExistingPeriodicWorkPolicy.UPDATE, req)
    }

    private fun stopPolling(ctx: Context) {
        runCatching { WorkManager.getInstance(ctx).cancelUniqueWork(POLL_WORK) }
    }

    fun disable(ctx: Context) {
        runCatching { UnifiedPush.unregister(ctx) }
        stopPolling(ctx)
        removeSubscription(ctx)
        PushStore.setMode(ctx, PushMode.OFF)
        PushStore.setError(ctx, null)
    }

    /** Retire l'abonnement de cet appareil côté serveur (déconnexion, changement de mode). */
    fun removeSubscription(ctx: Context) {
        val id = PushStore.subscriptionId(ctx) ?: return
        PushStore.setSubscriptionId(ctx, null)
        scope.launch { runCatching { Api(ctx).send("DELETE", "/api/user/push/subscriptions/$id") } }
    }

    /** Notification d'essai envoyée par le serveur (vérifie toute la chaîne). */
    suspend fun serverTest(ctx: Context): Int =
        Api(ctx).post<JsonObject>("/api/user/push/test")["sent"]?.jsonPrimitive?.content?.toIntOrNull() ?: 0

    /** Remet en place la vérification périodique après un redémarrage de l'application. */
    fun restore(ctx: Context) {
        if (PushStore.mode(ctx) == PushMode.POLLING) enablePolling(ctx)
    }
}
