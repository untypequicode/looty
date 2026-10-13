package io.github.untypequicode.looty.offline

import android.content.Context
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import io.github.untypequicode.looty.api.Api
import okhttp3.Request
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.File
import java.io.IOException
import java.security.MessageDigest

/**
 * Cache web hors ligne de la WebView : l'interface (HTML, scripts, styles), les lectures de l'API et
 * les fichiers du serveur sont demandés au serveur (réseau d'abord) et gardés sur l'appareil ; si le
 * serveur est injoignable, la copie est servie. Ainsi les pages web (éditeur de documents avec sa
 * copie locale Yjs, Moodle en lecture…) s'ouvrent sans connexion.
 *
 * Seules les lectures (GET) du serveur sont concernées ; redirections, envois, flux continus et
 * fichiers volumineux passent directement à la WebView.
 */
class WebCache(private val ctx: Context, private val api: Api) {
    private fun dir(): File {
        val server = runCatching { api.server }.getOrDefault("none")
        return File(ctx.filesDir, "webcache/" + Integer.toHexString(server.hashCode())).also { it.mkdirs() }
    }

    private fun key(url: String) = MessageDigest.getInstance("SHA-1").digest(url.toByteArray()).joinToString("") { "%02x".format(it) }

    private fun cacheable(request: WebResourceRequest): Boolean {
        val u = request.url
        if (request.method != "GET" || u.scheme?.lowercase() !in setOf("http", "https")) return false
        val server = runCatching { api.server }.getOrNull() ?: return false
        if (!u.toString().startsWith(server)) return false
        if (request.requestHeaders.keys.any { it.equals("Range", true) }) return false
        if (request.requestHeaders["Accept"]?.contains("text/event-stream") == true) return false
        val p = u.path.orEmpty()
        return !(p.startsWith("/__looty/") || p.startsWith("/api/auth/login/") || p.startsWith("/api/auth/desktop/") || p.contains("/stream") || p.startsWith("/collab") || p.startsWith("/ws"))
    }

    /** Réponse pour la WebView, ou null pour la laisser faire elle-même (hors cache, redirection…). */
    fun serve(request: WebResourceRequest): WebResourceResponse? {
        if (!cacheable(request)) return null
        val url = request.url.toString()
        // Fichiers dont le nom change avec le contenu (scripts, images, envois) : copie d’abord, sans réseau.
        val p = request.url.path.orEmpty()
        if (p.startsWith("/_nuxt/") || p.startsWith("/uploads/") || p.startsWith("/icons/")) fromCache(url)?.let { return it }
        val navigation = request.isForMainFrame && request.requestHeaders["Accept"]?.contains("text/html") == true
        val builder = Request.Builder().url(url)
        request.requestHeaders.forEach { (k, v) ->
            if (!k.equals("Accept-Encoding", true) && !k.equals("Cookie", true) && !k.equals("Host", true) && !k.equals("Range", true)) builder.header(k, v)
        }
        val resp = try {
            api.rawClient.newCall(builder.build()).execute()
        } catch (e: IOException) {
            val hit = fromCache(url) ?: if (navigation) fromCache("shell") else null
            android.util.Log.d("LootyWebCache", "offline " + (if (hit != null) "HIT " else "MISS ") + url + " (" + e.javaClass.simpleName + ")")
            return hit
        }
        resp.use { r ->
            if (r.isRedirect) return null
            if (r.code in 502..504) return fromCache(url) ?: if (navigation) fromCache("shell") else null
            val type = r.header("Content-Type") ?: "application/octet-stream"
            val mime = type.substringBefore(';').trim()
            val charset = Regex("charset=([^;]+)", RegexOption.IGNORE_CASE).find(type)?.groupValues?.get(1)?.trim('"', ' ')
            val length = r.body.contentLength()
            val attachment = r.header("Content-Disposition")?.contains("attachment", true) == true
            // Réponse non conservée : flux direct (erreurs, téléchargements, gros fichiers).
            if (!r.isSuccessful || attachment || length > MAX_BODY || length < 0 && !mime.startsWith("text/") && !mime.contains("json") && !mime.contains("javascript")) {
                val bytes = r.body.bytes()
                return WebResourceResponse(mime, charset, r.code, r.message.ifBlank { "OK" }, passHeaders(r), ByteArrayInputStream(bytes))
            }
            val bytes = r.body.bytes()
            if (bytes.size <= MAX_BODY) {
                store(url, mime, charset, bytes)
                if (navigation && mime == "text/html") store("shell", mime, charset, bytes)
            }
            return WebResourceResponse(mime, charset, r.code, r.message.ifBlank { "OK" }, passHeaders(r), ByteArrayInputStream(bytes))
        }
    }

    private fun passHeaders(r: okhttp3.Response): Map<String, String> =
        r.headers.toMultimap().filterKeys { it.lowercase() in setOf("cache-control", "content-language", "etag", "last-modified", "access-control-allow-origin") }.mapValues { it.value.joinToString(", ") }

    private fun store(name: String, mime: String, charset: String?, bytes: ByteArray) = synchronized(LOCK) {
        runCatching {
            val k = key(name)
            val body = File(dir(), "$k.body")
            val tmp = File(dir(), "$k.tmp")
            tmp.writeBytes(bytes); tmp.renameTo(body)
            File(dir(), "$k.meta").writeText(JSONObject().put("mime", mime).put("charset", charset ?: "").toString())
        }
    }

    private fun has(name: String) = File(dir(), key(name) + ".body").exists()

    private fun fromCache(name: String): WebResourceResponse? = synchronized(LOCK) {
        runCatching {
            val k = key(name)
            val body = File(dir(), "$k.body").takeIf { it.exists() } ?: return null
            val meta = JSONObject(File(dir(), "$k.meta").readText())
            WebResourceResponse(meta.getString("mime"), meta.optString("charset").ifEmpty { null }, 200, "OK", mapOf("X-Looty-Offline" to "1"), ByteArrayInputStream(body.readBytes()))
        }.getOrNull()
    }

    /**
     * Télécharge toute l'interface (liste fournie par /asset-manifest.json) et la page d'accueil, pour
     * ouvrir les pages web sans connexion, y compris les morceaux chargés seulement à la demande.
     * Les fichiers déjà gardés ne sont pas retéléchargés.
     */
    fun prefetchAssets() {
        val client = api.rawClient
        fun get(path: String): okhttp3.Response? = runCatching { client.newCall(Request.Builder().url(api.server + path).header("Accept", "*/*").build()).execute() }.getOrNull()
        val manifest = get("/asset-manifest.json")?.use { r -> if (r.isSuccessful) runCatching { JSONObject(r.body.string()) }.getOrNull() else null } ?: return
        val version = manifest.optString("version")
        val prefs = ctx.getSharedPreferences("looty", 0)
        if (version.isNotEmpty() && prefs.getString("assetVersion", null) == version) return
        val files = manifest.optJSONArray("files") ?: return
        // Page d'accueil (coquille de l'application, identique pour toutes les routes).
        client.newCall(Request.Builder().url(api.server + "/").header("Accept", "text/html").build()).runCatching { execute() }.getOrNull()?.use { r ->
            if (r.isSuccessful) { val b = r.body.bytes(); store("shell", "text/html", "utf-8", b); store(api.server + "/", "text/html", "utf-8", b) }
        }
        var missing = 0
        for (i in 0 until files.length()) {
            val path = files.getString(i)
            if (has(api.server + path)) continue
            val ok = get(path)?.use { r ->
                if (!r.isSuccessful || r.body.contentLength() > MAX_BODY) return@use false
                val type = r.header("Content-Type") ?: "application/octet-stream"
                store(api.server + path, type.substringBefore(';').trim(), Regex("charset=([^;]+)", RegexOption.IGNORE_CASE).find(type)?.groupValues?.get(1), r.body.bytes())
                true
            } ?: false
            if (!ok) missing++
        }
        // Version marquée complète seulement si tout est là : sinon nouvel essai au prochain passage.
        if (missing == 0) prefs.edit().putString("assetVersion", version).apply()
    }

    /** Garde environ 120 Mo : les fichiers les moins récents partent d'abord. */
    @Synchronized fun trim(maxBytes: Long = 120L * 1024 * 1024) {
        val files = File(ctx.filesDir, "webcache").walkTopDown().filter { it.isFile }.sortedBy { it.lastModified() }.toList()
        var total = files.sumOf { it.length() }
        for (f in files) { if (total <= maxBytes) break; total -= f.length(); f.delete() }
    }

    companion object {
        private const val MAX_BODY = 12L * 1024 * 1024
        private val LOCK = Any()
    }
}
