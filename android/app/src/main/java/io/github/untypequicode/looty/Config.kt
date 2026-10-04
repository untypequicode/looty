package io.github.untypequicode.looty

import android.content.Context
import android.net.Uri
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL

/** Réglages persistants : serveur choisi et connexion SSO en cours. */
object Config {
    private fun prefs(ctx: Context) = ctx.getSharedPreferences("looty", Context.MODE_PRIVATE)

    /** Origine du serveur (https://looty.example.org), sans barre finale. */
    fun server(ctx: Context): String? = prefs(ctx).getString("server", null)

    fun setServer(ctx: Context, origin: String) {
        prefs(ctx).edit().putString("server", origin).remove("sso").apply()
    }

    /** Connexion en cours dans le navigateur (survit à l'arrêt de l'application). */
    data class PendingSso(val state: String, val verifier: String, val redirect: String, val browserUrl: String, val createdAt: Long)

    fun pendingSso(ctx: Context): PendingSso? {
        val raw = prefs(ctx).getString("sso", null) ?: return null
        return runCatching {
            val o = JSONObject(raw)
            PendingSso(o.getString("state"), o.getString("verifier"), o.getString("redirect"), o.getString("browserUrl"), o.getLong("createdAt"))
        }.getOrNull()?.takeIf { System.currentTimeMillis() - it.createdAt < 15 * 60_000 }
    }

    fun setPendingSso(ctx: Context, p: PendingSso?) {
        val e = prefs(ctx).edit()
        if (p == null) e.remove("sso") else e.putString(
            "sso",
            JSONObject().put("state", p.state).put("verifier", p.verifier).put("redirect", p.redirect).put("browserUrl", p.browserUrl).put("createdAt", p.createdAt).toString(),
        )
        e.apply()
    }

    /** « looty.example.org » → « https://looty.example.org » ; null si l'adresse est invalide. */
    fun normalize(input: String): String? {
        var s = input.trim().trimEnd('/')
        if (s.isEmpty()) return null
        if (!s.contains("://")) s = "https://$s"
        val uri = Uri.parse(s)
        val scheme = uri.scheme?.lowercase() ?: return null
        val host = uri.host?.lowercase() ?: return null
        if (scheme != "https" && scheme != "http") return null
        val port = if (uri.port > 0) ":${uri.port}" else ""
        return "$scheme://$host$port"
    }

    /** http:// n'est permis que vers un serveur local (cf. network_security.xml). */
    fun cleartextAllowed(origin: String): Boolean {
        val host = Uri.parse(origin).host ?: return false
        return host == "localhost" || host == "10.0.2.2" || host.endsWith(".local") || host.endsWith(".lan")
    }

    sealed interface Health {
        data class Ok(val androidSso: Boolean) : Health
        data object NotLooty : Health
        data object Unreachable : Health
    }

    /** GET /api/health (à appeler hors du fil principal). */
    fun health(origin: String, timeoutMs: Int = 8000): Health {
        return try {
            val c = URL("$origin/api/health").openConnection() as HttpURLConnection
            c.connectTimeout = timeoutMs
            c.readTimeout = timeoutMs
            c.instanceFollowRedirects = true
            try {
                if (c.responseCode != 200) return Health.NotLooty
                val o = JSONObject(c.inputStream.bufferedReader().readText())
                if (o.optString("status") != "ok") Health.NotLooty else Health.Ok(o.optBoolean("android_sso"))
            } finally {
                c.disconnect()
            }
        } catch (_: org.json.JSONException) {
            Health.NotLooty
        } catch (_: Exception) {
            Health.Unreachable
        }
    }
}
