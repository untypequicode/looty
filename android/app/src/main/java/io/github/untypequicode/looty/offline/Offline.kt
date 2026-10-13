package io.github.untypequicode.looty.offline

import android.content.Context
import io.github.untypequicode.looty.Config
import io.github.untypequicode.looty.api.ApiException
import kotlinx.serialization.Serializable
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
class Outbox(ctx: Context, private val json: Json) {
    private val file = File(ctx.filesDir, "outbox.json")
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
    @Synchronized fun remove(id: String) { items.removeAll { it.id == id }; save() }
    @Synchronized fun clear() { items.clear(); file.delete() }
}
