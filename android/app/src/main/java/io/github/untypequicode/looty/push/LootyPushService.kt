package io.github.untypequicode.looty.push

import android.os.Build
import io.github.untypequicode.looty.Instances
import io.github.untypequicode.looty.api.Api
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import org.json.JSONObject
import org.unifiedpush.android.connector.FailedReason
import org.unifiedpush.android.connector.PushService
import org.unifiedpush.android.connector.data.PushEndpoint
import org.unifiedpush.android.connector.data.PushMessage

/** Reçoit les évènements UnifiedPush : adresse d'envoi, messages, erreurs. */
class LootyPushService : PushService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** Nouvelle adresse : elle est communiquée au serveur Looty (avec les clés de chiffrement). */
    override fun onNewEndpoint(endpoint: PushEndpoint, instance: String) {
        val ctx = applicationContext
        if (Instances.origin(ctx, instance) == null) return
        val keys = endpoint.pubKeySet
        if (keys == null) {
            PushStore.setError(ctx, "keys", instance)
            return
        }
        scope.launch {
            val previous = PushStore.subscriptionId(ctx, instance)
            runCatching {
                val body = buildJsonObject {
                    put("endpoint", endpoint.url)
                    put("keys", buildJsonObject { put("p256dh", keys.pubKey); put("auth", keys.auth) })
                    put("device", "Android · ${Build.MANUFACTURER} ${Build.MODEL}".take(120))
                }
                val id = Api(ctx, instance).post<JsonObject>("/api/user/push/subscriptions", body)["id"]?.jsonPrimitive?.content
                PushStore.setSubscriptionId(ctx, id, instance)
                PushStore.setMode(ctx, PushMode.UNIFIED_PUSH, instance)
                PushStore.setError(ctx, null, instance)
                // L'adresse a changé : l'ancien abonnement n'a plus d'objet.
                if (previous != null && previous != id) runCatching { Api(ctx, instance).send("DELETE", "/api/user/push/subscriptions/$previous") }
            }.onFailure { PushStore.setError(ctx, "server", instance) }
        }
    }

    /** Message du serveur Looty (déchiffré par la bibliothèque) : { title, body, url, tag }. */
    override fun onMessage(message: PushMessage, instance: String) {
        if (Instances.origin(applicationContext, instance) == null) return
        val o = runCatching { JSONObject(String(message.content, Charsets.UTF_8)) }.getOrNull()
        if (o == null || !message.decrypted) return
        Notifier.show(
            applicationContext,
            title = o.optString("title").ifBlank { "Looty" },
            body = o.optString("body"),
            path = o.optString("url").ifBlank { null },
            tag = o.optString("tag").ifBlank { null },
            kind = Notifier.Kind.of(o.optString("kind").ifBlank { null }),
            instance = instance,
        )
    }

    override fun onRegistrationFailed(reason: FailedReason, instance: String) {
        PushStore.setError(applicationContext, reason.name, instance)
    }

    /** Le distributeur a retiré l'enregistrement : plus de push (l'abonnement du serveur est retiré). */
    override fun onUnregistered(instance: String) {
        val ctx = applicationContext
        Push.removeSubscription(ctx, instance)
        // Plus de push : retour à la vérification périodique pour ne pas perdre les notifications.
        if (PushStore.mode(ctx, instance) == PushMode.UNIFIED_PUSH) Push.enablePolling(ctx, instance)
    }
}
