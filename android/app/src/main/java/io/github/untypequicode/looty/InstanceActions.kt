package io.github.untypequicode.looty

import android.content.Context
import android.webkit.CookieManager
import io.github.untypequicode.looty.api.Api
import io.github.untypequicode.looty.offline.OfflineCache
import io.github.untypequicode.looty.offline.Outbox
import io.github.untypequicode.looty.push.Push
import io.github.untypequicode.looty.push.PushStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Actions sur une instance qui touchent tout ce qu'elle garde sur l'appareil (ADR 0017). */
object InstanceActions {
    /** Réglages d'une instance (AppState, widgets, tâches de fond) : le reste du fichier « looty » est commun à l'appareil. */
    private val OWN_KEYS = setOf("workspace", "taskScope", "lastBoard", "boardView", "hiddenCalendars", "mailAccount", "assetVersion", "refusedChanges")

    /** Cookies de cette adresse seulement : les autres instances restent connectées. */
    fun clearCookies(origin: String) {
        val cm = CookieManager.getInstance()
        val host = android.net.Uri.parse(origin).host
        cm.getCookie(origin)?.split(';')?.map { it.substringBefore('=').trim() }?.filter { it.isNotEmpty() }?.forEach { name ->
            cm.setCookie(origin, "$name=; Max-Age=0; Path=/")
            if (host != null) cm.setCookie(origin, "$name=; Max-Age=0; Path=/; Domain=$host")
        }
        cm.flush()
    }

    /** Déconnecte l'instance [id] (serveur, notifications, cookies, copie hors ligne, file d'attente) sans la retirer. */
    suspend fun signOut(ctx: Context, id: String) {
        val origin = Instances.origin(ctx, id) ?: return
        val api = Api(ctx, id)
        Push.disable(ctx, id)
        runCatching { api.send("POST", "/api/auth/logout") }
        withContext(Dispatchers.IO) {
            clearCookies(origin)
            OfflineCache(ctx, id).clear()
            Outbox.get(ctx, api.json, id).clear()
            val prefs = Instances.prefs(ctx, id)
            val e = prefs.edit().remove("assetVersion")
            prefs.all.keys.filter { it.startsWith("studyRead-") }.forEach { e.remove(it) }
            e.apply()
        }
    }

    /**
     * Retire l'instance [id] et tout ce qu'elle a laissé : réglages, notifications (abonnement, canaux, tâche de fond),
     * copie hors ligne, file d'attente. Les widgets qui la montraient affichent « déconnecté ». Renvoie l'instance active ensuite.
     */
    suspend fun remove(ctx: Context, id: String): Instance? {
        signOut(ctx, id)
        withContext(Dispatchers.IO) {
            Outbox.forget(id)
            PushStore.clear(ctx, id)
            // L'instance d'origine garde ses réglages dans le fichier « looty » commun : seules ses clés partent.
            if (id != InstanceStore.DEFAULT) {
                ctx.deleteSharedPreferences(InstanceStore.scoped("looty", id))
                ctx.deleteSharedPreferences(InstanceStore.scoped("looty.widget", id))
            } else {
                val p = Instances.prefs(ctx, id)
                p.edit().also { e -> p.all.keys.filter { it in OWN_KEYS || it.startsWith("studyRead-") }.forEach(e::remove) }.apply()
                ctx.deleteSharedPreferences("looty.widget")
            }
        }
        val next = Instances.store(ctx).remove(id)
        // Les canaux de l'instance disparaissent avec elle.
        io.github.untypequicode.looty.push.Notifier.ensureChannels(ctx)
        return next
    }
}
