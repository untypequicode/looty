package io.github.untypequicode.looty.push

import android.app.Activity
import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import io.github.untypequicode.looty.InstanceStore
import io.github.untypequicode.looty.Instances
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
    private fun pollWork(inst: String) = InstanceStore.scoped("looty-poll", inst)

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
        val inst = Instances.activeId(activity)
        PushStore.setError(activity, null, inst)
        UnifiedPush.tryUseCurrentOrDefaultDistributor(activity) { ok ->
            if (!ok) return@tryUseCurrentOrDefaultDistributor done(false)
            registerNow(activity.applicationContext, inst, done)
        }
    }

    /** Autre application de distribution. */
    fun pickDistributor(activity: Activity, done: (Boolean) -> Unit) {
        val inst = Instances.activeId(activity)
        UnifiedPush.tryPickDistributor(activity) { ok ->
            if (!ok) return@tryPickDistributor done(false)
            registerNow(activity.applicationContext, inst, done)
        }
    }

    /** Enregistrement auprès du distributeur, limité à la clé VAPID de ce serveur. */
    private fun registerNow(ctx: Context, inst: String, done: (Boolean) -> Unit) {
        scope.launch {
            val vapid = runCatching { Api(ctx, inst).get<JsonObject>("/api/user/push/key")["key"]?.jsonPrimitive?.content }.getOrNull()
            if (vapid == null) {
                PushStore.setError(ctx, "server", inst)
                return@launch kotlinx.coroutines.withContext(Dispatchers.Main) { done(false) }
            }
            val ok = runCatching { UnifiedPush.register(ctx, instance = inst, messageForDistributor = "Looty", vapid = vapid) }.isSuccess
            if (ok) {
                PushStore.setMode(ctx, PushMode.UNIFIED_PUSH, inst)
                stopPolling(ctx, inst)
            } else PushStore.setError(ctx, "vapid", inst)
            kotlinx.coroutines.withContext(Dispatchers.Main) { done(ok) }
        }
    }

    /** Vérification périodique (plus tôt que 15 minutes n'est pas permis par Android). */
    fun enablePolling(ctx: Context, inst: String = Instances.activeId(ctx)) {
        runCatching { UnifiedPush.unregister(ctx, inst) }
        removeSubscription(ctx, inst)
        PushStore.setMode(ctx, PushMode.POLLING, inst)
        PushStore.setError(ctx, null, inst)
        PushStore.setLastPoll(ctx, System.currentTimeMillis(), inst)
        val req = PeriodicWorkRequestBuilder<PollWorker>(15, TimeUnit.MINUTES)
            .setInputData(androidx.work.workDataOf("instance" to inst))
            .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
            .build()
        WorkManager.getInstance(ctx).enqueueUniquePeriodicWork(pollWork(inst), ExistingPeriodicWorkPolicy.UPDATE, req)
    }

    private fun stopPolling(ctx: Context, inst: String) {
        runCatching { WorkManager.getInstance(ctx).cancelUniqueWork(pollWork(inst)) }
    }

    fun disable(ctx: Context, inst: String = Instances.activeId(ctx)) {
        runCatching { UnifiedPush.unregister(ctx, inst) }
        stopPolling(ctx, inst)
        removeSubscription(ctx, inst)
        PushStore.setMode(ctx, PushMode.OFF, inst)
        PushStore.setError(ctx, null, inst)
    }

    /** Retire l'abonnement de cet appareil côté serveur (déconnexion, changement de mode). */
    fun removeSubscription(ctx: Context, inst: String = Instances.activeId(ctx)) {
        val id = PushStore.subscriptionId(ctx, inst) ?: return
        PushStore.setSubscriptionId(ctx, null, inst)
        scope.launch { runCatching { Api(ctx, inst).send("DELETE", "/api/user/push/subscriptions/$id") } }
    }

    /** Notification d'essai envoyée par le serveur (vérifie toute la chaîne). */
    suspend fun serverTest(ctx: Context): Int =
        Api(ctx).post<JsonObject>("/api/user/push/test")["sent"]?.jsonPrimitive?.content?.toIntOrNull() ?: 0

    /** Remet en place la vérification périodique après un redémarrage de l'application. */
    fun restore(ctx: Context) {
        for (i in Instances.all(ctx)) when (PushStore.mode(ctx, i.id)) {
            PushMode.POLLING -> enablePolling(ctx, i.id)
            // UnifiedPush actif : la vérification périodique est inutile (économie de batterie et de réseau).
            PushMode.UNIFIED_PUSH -> stopPolling(ctx, i.id)
            PushMode.OFF -> {}
        }
    }
}
