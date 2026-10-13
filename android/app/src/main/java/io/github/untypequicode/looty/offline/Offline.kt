package io.github.untypequicode.looty.offline

import android.content.Context
import io.github.untypequicode.looty.Config
import io.github.untypequicode.looty.api.ApiException
import androidx.glance.appwidget.updateAll
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import io.github.untypequicode.looty.api.Api
import kotlinx.coroutines.sync.Mutex
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File
import java.io.IOException

/** Le serveur est injoignable (pas de réseau, délai dépassé, passerelle en erreur) : pas un refus du serveur. */
fun Throwable.isOffline(): Boolean = this is IOException || (this is ApiException && status in 502..504)

/**
 * Copie des données sur l'appareil (JSON), par serveur : les écrans s'affichent depuis elle quand
 * le serveur est injoignable. Écriture atomique (fichier temporaire puis renommage).
 */
class OfflineCache(private val ctx: Context) {
    private fun dir(): File {
        val server = Config.server(ctx) ?: "none"
        return File(ctx.filesDir, "offline/" + Integer.toHexString(server.hashCode())).also { it.mkdirs() }
    }

    private fun file(key: String, ext: String) =
        File(dir(), key.replace(Regex("[^A-Za-z0-9._-]"), "_").takeLast(100) + "-" + Integer.toHexString(key.hashCode()) + ext)

    @Synchronized fun put(key: String, text: String) {
        val f = file(key, ".json")
        val tmp = File(f.path + ".tmp")
        runCatching { tmp.writeText(text); tmp.renameTo(f) }
    }

    @Synchronized fun get(key: String): String? = file(key, ".json").takeIf { it.exists() }?.runCatching { readText() }?.getOrNull()

    @Synchronized fun putBytes(key: String, bytes: ByteArray) {
        if (bytes.size > 3_000_000) return
        runCatching { file(key, ".bin").writeBytes(bytes) }
    }

    @Synchronized fun getBytes(key: String): ByteArray? = file(key, ".bin").takeIf { it.exists() }?.runCatching { readBytes() }?.getOrNull()

    /** Garde environ 80 Mo : les fichiers les plus anciens partent d'abord. */
    @Synchronized fun trim(maxBytes: Long = 80L * 1024 * 1024) {
        val files = File(ctx.filesDir, "offline").walkTopDown().filter { it.isFile }.sortedBy { it.lastModified() }.toList()
        var total = files.sumOf { it.length() }
        for (f in files) { if (total <= maxBytes) break; total -= f.length(); f.delete() }
    }

    @Synchronized fun clear() { File(ctx.filesDir, "offline").deleteRecursively() }
}

/** Modification faite hors ligne, rejouée dans l'ordre dès que le serveur répond. */
@Serializable
data class Pending(val id: String, val method: String, val path: String, val body: String? = null, val label: String)

/** File d'attente des modifications, gardée sur l'appareil (elle survit à la fermeture de l'application). */
class Outbox private constructor(ctx: Context, private val json: Json) {
    companion object {
        @Volatile private var instance: Outbox? = null
        /** Une seule file par processus : l'application et la tâche de fond partagent le même état. */
        fun get(ctx: Context, json: Json): Outbox = instance ?: synchronized(this) { instance ?: Outbox(ctx.applicationContext, json).also { instance = it } }
        /** Un seul rejeu à la fois (application ou tâche de fond), sinon une modification partirait deux fois. */
        val replayLock = Mutex()
    }

    private val file = File(ctx.filesDir, "outbox.json")
    /** Fichiers partagés hors ligne, en attente d'envoi (un fichier par modification « UPLOAD »). */
    val files = File(ctx.filesDir, "outbox-files")
    private var items: MutableList<Pending> = load()

    private fun load(): MutableList<Pending> =
        runCatching { json.decodeFromString<List<Pending>>(file.readText()).toMutableList() }.getOrDefault(mutableListOf())

    private fun save() {
        val tmp = File(file.path + ".tmp")
        runCatching { tmp.writeText(json.encodeToString<List<Pending>>(items)); tmp.renameTo(file) }
    }

    @Synchronized fun size() = items.size
    @Synchronized fun first(): Pending? = items.firstOrNull()
    @Synchronized fun all(): List<Pending> = items.toList()
    @Synchronized fun add(p: Pending) { items.add(p); save() }
    @Synchronized fun remove(id: String) { items.removeAll { it.id == id }; save(); File(files, id).delete() }
    @Synchronized fun clear() { items.clear(); file.delete(); files.deleteRecursively() }
}

/** Résultat d'un rejeu de la file. */
class ReplayResult(val sent: Boolean, val refused: List<String>, val offline: Boolean)

/**
 * Envoie les modifications en attente, dans l'ordre. S'arrête dès que le serveur est injoignable ;
 * un refus du serveur (4xx) écarte la modification et la signale.
 */
suspend fun Outbox.replay(api: Api): ReplayResult {
    var sent = false
    var offline = false
    val refused = mutableListOf<String>()
    while (true) {
        val p = first() ?: break
        try {
            if (p.method == "UPLOAD") {
                val meta = api.json.parseToJsonElement(p.body ?: "{}").jsonObject
                val f = File(files, p.id)
                if (f.exists()) api.upload(p.path, meta["name"]?.jsonPrimitive?.content ?: "file", meta["mime"]?.jsonPrimitive?.content ?: "application/octet-stream", f.readBytes())
            } else api.raw(p.method, p.path, p.body)
            remove(p.id); sent = true
        } catch (e: Exception) {
            if (e.isOffline()) { offline = true; break }
            remove(p.id)
            refused += p.label
        }
    }
    return ReplayResult(sent, refused, offline)
}

/** Tâche de fond : renvoie la file dès que le réseau est là, même si l'application est fermée. */
class OutboxWorker(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {
    override suspend fun doWork(): Result {
        val api = Api(applicationContext)
        val outbox = Outbox.get(applicationContext, api.json)
        if (outbox.size() == 0) return Result.success()
        if (!Outbox.replayLock.tryLock()) return Result.retry()
        try {
            val r = outbox.replay(api)
            // Refus à signaler à la prochaine ouverture de l'application.
            if (r.refused.isNotEmpty()) {
                val prefs = applicationContext.getSharedPreferences("looty", 0)
                prefs.edit().putStringSet("refusedChanges", prefs.getStringSet("refusedChanges", emptySet()).orEmpty() + r.refused).apply()
            }
            if (r.sent) runCatching { io.github.untypequicode.looty.widget.LootyWidget().updateAll(applicationContext) }
            return if (r.offline) Result.retry() else Result.success()
        } finally {
            Outbox.replayLock.unlock()
        }
    }

    companion object {
        /** À appeler quand une modification est mise en file : envoi dès que le réseau est disponible. */
        fun schedule(ctx: Context) {
            val req = OneTimeWorkRequestBuilder<OutboxWorker>().setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()).build()
            WorkManager.getInstance(ctx).enqueueUniqueWork("looty-outbox", ExistingWorkPolicy.REPLACE, req)
        }
    }
}
