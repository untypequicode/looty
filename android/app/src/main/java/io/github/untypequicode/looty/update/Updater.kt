package io.github.untypequicode.looty.update

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.net.Uri
import android.os.Build
import android.provider.Settings
import io.github.untypequicode.looty.BuildConfig
import io.github.untypequicode.looty.api.Api
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

/** Nouvelle version de l'application, annoncée par le site de téléchargement. */
data class Update(val version: String, val apkUrl: String, val title: String?, val notes: List<String>, val sha256: String? = null)

/** Ni site de téléchargement ni publication GitHub connue sur ce serveur : rien à vérifier. */
class NoUpdateSource : Exception("no_source")

/**
 * Mise à jour de l'application sans boutique : le serveur Looty indique l'adresse du site de
 * téléchargement du projet (`site` dans /api/releases) ; le site publie la dernière version dans
 * /latest.json. L'APK est téléchargé depuis ce site, sa somme SHA-256 est vérifiée, puis il est
 * installé par le système, qui refuse tout APK qui n'est pas signé avec la même clé que l'application.
 */
object Updater {
    private const val ACTION_RESULT = "io.github.untypequicode.looty.UPDATE_RESULT"

    /** Version installée, sans le suffixe des variantes de test (« -debug », « -qa »). */
    val current: String get() = BuildConfig.VERSION_NAME.substringBefore('-')

    fun newer(a: String, b: String): Boolean {
        val x = a.split('.').map { it.toIntOrNull() ?: 0 }
        val y = b.split('.').map { it.toIntOrNull() ?: 0 }
        for (i in 0 until maxOf(x.size, y.size)) {
            val d = x.getOrElse(i) { 0 } - y.getOrElse(i) { 0 }
            if (d != 0) return d > 0
        }
        return false
    }

    /** Adresse du site : https obligatoire (http seulement pour un site local, en test). */
    private fun acceptable(site: String, debug: Boolean): Boolean {
        val u = Uri.parse(site)
        return u.scheme == "https" || (debug && u.scheme == "http" && (u.host == "10.0.2.2" || u.host == "localhost"))
    }

    /** Dernière version publiée si elle est plus récente que celle installée, sinon null. */
    suspend fun check(ctx: Context, api: Api, lang: String): Update? = withContext(Dispatchers.IO) {
        // Essais (variante debug seulement) : source de mise à jour imposée dans les préférences.
        if (BuildConfig.DEBUG) {
            ctx.getSharedPreferences("looty", 0).getString("updateOverride", null)?.let { raw ->
                val o = api.json.parseToJsonElement(raw).jsonObject
                val v = o["version"]?.jsonPrimitive?.content ?: return@withContext null
                return@withContext if (newer(v, current)) Update(v, o["apk"]!!.jsonPrimitive.content, "Test", listOf("Test update"), o["sha256"]?.jsonPrimitive?.content) else null
            }
        }
        val root = api.json.parseToJsonElement(api.raw("GET", "/api/releases")).jsonObject
        val site = root["site"]?.takeIf { it !is kotlinx.serialization.json.JsonNull }?.jsonPrimitive?.content?.trimEnd('/')
        val client = OkHttpClient.Builder().connectTimeout(15, TimeUnit.SECONDS).readTimeout(30, TimeUnit.SECONDS).build()
        // Sans site de téléchargement : la publication GitHub du projet (dépôt public).
        if (site.isNullOrEmpty() || !acceptable(site, BuildConfig.DEBUG)) return@withContext fromGithub(root, client, api, lang) 
        val fromSite = fromSite(site, client, api, lang)
        return@withContext fromSite
    }

    /** Dernière version annoncée par le site (`/latest.json`), si elle est plus récente que celle installée. */
    private fun fromSite(site: String, client: OkHttpClient, api: Api, lang: String): Update? {
        val meta = client.newCall(Request.Builder().url("$site/latest.json").header("Accept", "application/json").build()).execute().use { r ->
            if (!r.isSuccessful) throw java.io.IOException("HTTP " + r.code)
            api.json.parseToJsonElement(r.body.string()).jsonObject
        }
        val android: JsonObject = meta["android"]?.jsonObject ?: return null
        val version = android["version"]?.jsonPrimitive?.content ?: return null
        if (!newer(version, current)) return null
        // L'APK doit venir du site lui-même.
        val apk = android["url"]?.jsonPrimitive?.content ?: return null
        if (!apk.startsWith("$site/")) return null
        val notes = (android["notes"]?.jsonObject?.get(lang) ?: android["notes"]?.jsonObject?.get("en"))?.jsonArray?.map { it.jsonPrimitive.content } ?: emptyList()
        val title = android["title"]?.jsonObject?.let { t -> (t[lang] ?: t["en"])?.jsonPrimitive?.content }
        return Update(version, apk, title, notes, android["sha256"]?.jsonPrimitive?.content?.lowercase())
    }

    private val TAG_URL = Regex("^https://github\\.com/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)/releases/tag/android-v([0-9]+(?:\\.[0-9]+){1,3})$")

    /** Publication GitHub (dépôt public) annoncée par /api/releases : APK `looty-<version>.apk`, somme SHA-256 de l'API GitHub si elle la donne. */
    private fun fromGithub(root: JsonObject, client: OkHttpClient, api: Api, lang: String): Update? {
        val android = root["latest"]?.jsonObject?.get("android")?.jsonObject ?: throw NoUpdateSource()
        val version = android["version"]?.jsonPrimitive?.content ?: throw NoUpdateSource()
        val page = android["url"]?.jsonPrimitive?.content ?: throw NoUpdateSource()
        val m = TAG_URL.matchEntire(page)
        if (m == null || m.groupValues[3] != version) throw NoUpdateSource()
        if (!newer(version, current)) return null
        val apk = page.replace("/releases/tag/", "/releases/download/") + "/looty-$version.apk"
        // Somme SHA-256 publiée par GitHub pour ce fichier (absente : seule la signature d'Android protège).
        val sha = runCatching {
            client.newCall(Request.Builder().url("https://api.github.com/repos/${m.groupValues[1]}/${m.groupValues[2]}/releases/tags/android-v$version").header("Accept", "application/vnd.github+json").build()).execute().use { r ->
                if (!r.isSuccessful) return@use null
                api.json.parseToJsonElement(r.body.string()).jsonObject["assets"]?.jsonArray
                    ?.map { it.jsonObject }?.firstOrNull { it["name"]?.jsonPrimitive?.content == "looty-$version.apk" }
                    ?.get("digest")?.jsonPrimitive?.content?.removePrefix("sha256:")?.lowercase()
            }
        }.getOrNull()
        val entries = root["releases"]?.jsonArray ?: return Update(version, apk, null, emptyList(), sha)
        val mine = entries.map { it.jsonObject }.filter { it["platform"]?.jsonPrimitive?.content == "android" && newer(it["version"]?.jsonPrimitive?.content ?: "0", current) }
        val notes = mine.flatMap { e -> (e["notes"]?.jsonObject?.get(lang) ?: e["notes"]?.jsonObject?.get("en"))?.jsonArray?.map { n -> n.jsonPrimitive.content } ?: emptyList() }
        val title = mine.firstOrNull()?.get("title")?.jsonObject?.let { t -> (t[lang] ?: t["en"])?.jsonPrimitive?.content }
        return Update(version, apk, title, notes, sha)
    }

    /** Télécharge l'APK dans le cache de l'application ; `onProgress` reçoit une fraction (0..1) ou -1 si inconnue. */
    suspend fun download(ctx: Context, update: Update, onProgress: (Float) -> Unit): File = withContext(Dispatchers.IO) {
        val dir = File(ctx.cacheDir, "updates").also { it.mkdirs(); it.listFiles()?.forEach { f -> f.delete() } }
        val out = File(dir, "looty-${update.version}.apk")
        val client = OkHttpClient.Builder().connectTimeout(20, TimeUnit.SECONDS).readTimeout(60, TimeUnit.SECONDS).build()
        client.newCall(Request.Builder().url(update.apkUrl).build()).execute().use { r ->
            if (!r.isSuccessful) throw java.io.IOException("HTTP " + r.code)
            val total = r.body.contentLength()
            var done = 0L
            val digest = MessageDigest.getInstance("SHA-256")
            r.body.byteStream().use { input -> out.outputStream().use { o ->
                val buf = ByteArray(64 * 1024)
                while (true) {
                    val n = input.read(buf)
                    if (n < 0) break
                    o.write(buf, 0, n); digest.update(buf, 0, n); done += n
                    onProgress(if (total > 0) done.toFloat() / total else -1f)
                }
            } }
            // Fichier altéré ou incomplet : on ne l'installe pas.
            val sum = digest.digest().joinToString("") { "%02x".format(it) }
            if (update.sha256 != null && sum != update.sha256) { out.delete(); throw java.io.IOException("checksum") }
        }
        out
    }

    /** L'installation d'applications inconnues est-elle autorisée pour Looty ? Sinon, ouvre le réglage. */
    fun ensureCanInstall(ctx: Context): Boolean {
        if (ctx.packageManager.canRequestPackageInstalls()) return true
        ctx.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + ctx.packageName)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        return false
    }

    /** Confie l'APK à l'installateur du système ; le résultat arrive à [InstallReceiver]. */
    fun install(ctx: Context, apk: File) {
        val installer = ctx.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
            setAppPackageName(ctx.packageName)
            // Android 12+ : mise à jour sans nouvelle confirmation quand elle est permise (Looty déjà installé par Looty).
            if (Build.VERSION.SDK_INT >= 31) setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
        }
        val id = installer.createSession(params)
        installer.openSession(id).use { s ->
            apk.inputStream().use { i -> s.openWrite("looty.apk", 0, apk.length()).use { o -> i.copyTo(o); s.fsync(o) } }
            val intent = Intent(ctx, InstallReceiver::class.java).setAction(ACTION_RESULT)
            // Modifiable : le système y ajoute le statut de l'installation.
            val pending = PendingIntent.getBroadcast(ctx, id, intent, PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
            s.commit(pending.intentSender)
        }
    }
}
