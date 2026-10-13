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
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.util.concurrent.TimeUnit

/** Nouvelle version de l'application, annoncée par le serveur (notes de version) et publiée sur GitHub. */
data class Update(val version: String, val apkUrl: String, val title: String?, val notes: List<String>)

/**
 * Mise à jour de l'application sans boutique : le serveur indique la dernière version Android et sa
 * page de publication GitHub ; l'APK est téléchargé depuis cette publication puis installé par le
 * système, qui refuse tout APK qui n'est pas signé avec la même clé que l'application installée.
 */
object Updater {
    private const val ACTION_RESULT = "io.github.untypequicode.looty.UPDATE_RESULT"
    private val TAG_URL = Regex("^https://github\\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/releases/tag/android-v([0-9]+(?:\\.[0-9]+){1,3})$")

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

    /** Dernière version publiée si elle est plus récente que celle installée, sinon null. */
    suspend fun check(ctx: Context, api: Api, lang: String): Update? = withContext(Dispatchers.IO) {
        // Essais (variante debug seulement) : source de mise à jour imposée dans les préférences.
        if (BuildConfig.DEBUG) {
            ctx.getSharedPreferences("looty", 0).getString("updateOverride", null)?.let { raw ->
                val o = api.json.parseToJsonElement(raw).jsonObject
                val v = o["version"]?.jsonPrimitive?.content ?: return@withContext null
                return@withContext if (newer(v, current)) Update(v, o["apk"]!!.jsonPrimitive.content, "Test", listOf("Test update")) else null
            }
        }
        val root = api.json.parseToJsonElement(api.raw("GET", "/api/releases")).jsonObject
        val android = root["latest"]?.jsonObject?.get("android")?.jsonObject ?: return@withContext null
        val version = android["version"]?.jsonPrimitive?.content ?: return@withContext null
        val page = android["url"]?.jsonPrimitive?.content ?: return@withContext null
        if (!newer(version, current)) return@withContext null
        // Seules les publications GitHub au format « android-v<version> » sont acceptées.
        val m = TAG_URL.matchEntire(page) ?: return@withContext null
        if (m.groupValues[1] != version) return@withContext null
        val apk = page.replace("/releases/tag/", "/releases/download/") + "/looty-$version.apk"
        // Notes de la version (et des précédentes non installées), dans la langue de l'appareil.
        val entries: JsonArray = root["releases"]?.jsonArray ?: JsonArray(emptyList())
        val mine = entries.map { it.jsonObject }.filter { it["platform"]?.jsonPrimitive?.content == "android" && newer(it["version"]?.jsonPrimitive?.content ?: "0", current) }
        val notes = mine.flatMap { e -> (e["notes"]?.jsonObject?.get(lang) ?: e["notes"]?.jsonObject?.get("en"))?.jsonArray?.map { n -> n.jsonPrimitive.content } ?: emptyList() }
        val title = mine.firstOrNull()?.get("title")?.jsonObject?.let { t -> (t[lang] ?: t["en"])?.jsonPrimitive?.content }
        Update(version, apk, title, notes)
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
            r.body.byteStream().use { input -> out.outputStream().use { o ->
                val buf = ByteArray(64 * 1024)
                while (true) {
                    val n = input.read(buf)
                    if (n < 0) break
                    o.write(buf, 0, n); done += n
                    onProgress(if (total > 0) done.toFloat() / total else -1f)
                }
            } }
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
