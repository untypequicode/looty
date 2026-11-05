package io.github.untypequicode.looty

import java.net.URI
import java.net.URLDecoder

/** Liens looty://w/… reçus de la bannière web, d'un partage, d'une notification ou d'un widget. */
object InstanceLinks {
    sealed interface LinkAction {
        /** Page [path] (/w/…) de l'instance de l'app. */
        data class Open(val path: String) : LinkAction
        /** Lien d'une autre instance que celle de l'app : proposé dans le navigateur ([webUrl]). */
        data class Foreign(val linkInstance: String, val appInstance: String, val webUrl: String) : LinkAction
        /** L'app n'a pas d'instance : se connecter à [instance] (adresse préremplie), puis ouvrir [path]. */
        data class Connect(val instance: String, val path: String) : LinkAction
    }

    /** Pages /w/<espace>/<page> qui ont un écran natif du même nom. */
    private val SCREENS = setOf("docs", "inbox", "calendar", "announcements", "drive", "moodle", "activity", "trash", "graph", "projects", "mail")

    /** Sections des réglages qui ont un écran natif. */
    private val SETTINGS = setOf("profile", "preferences", "security", "calendars", "workspace", "members")

    /** Écran d'une page /w/… : natif quand il existe, web sinon. */
    sealed interface Screen {
        /** Route de navigation de l'app, dans l'espace [workspace]. */
        data class Native(val workspace: String, val route: String) : Screen
        data class Web(val path: String) : Screen
    }

    /** « a=1&b » → [« a=1 », « b »] (valeurs encore encodées). */
    private fun params(query: String) = query.split('&').filter { it.isNotEmpty() }

    /** Décodage d'une valeur de requête ; null si un échappement est invalide (« %zz »), sans exception. */
    private fun decode(s: String): String? = runCatching { URLDecoder.decode(s.replace("+", "%2B"), "UTF-8") }.getOrNull()

    /** Lien de l'app (looty://w/…) vers la page [path] (/w/…), sans instance : celle de l'app. */
    fun appLink(path: String) = "looty:/$path"

    fun screen(path: String): Screen {
        // Découpage à la main : java.net.URI refuse des liens que le navigateur laisse passer.
        val bare = path.substringBefore('#')
        val seg = bare.substringBefore('?').split('/').filter { it.isNotEmpty() }
        val ws = seg.getOrNull(1)
        if (seg.firstOrNull() != "w" || ws == null) return Screen.Web(path)
        val query = params(bare.substringAfter('?', "")).associate { it.substringBefore('=') to it.substringAfter('=', "").let { v -> decode(v) ?: v } }
        val page = seg.getOrNull(2)
        val id = seg.getOrNull(3)
        val task = query["task"]?.takeIf { it.isNotEmpty() }
        val route = when {
            page == "task" && id != null -> "task/$id"
            (page == "board" || page == "my-tasks") && task != null -> "task/$task"
            page == "board" && id != null -> "board/$id"
            page == "doc" && id != null && "comment" in query -> "doc/$id/comments"
            page == "doc" && id != null && "history" in query -> "doc/$id/history"
            page == "doc" && id != null || page == "read" && id != null -> "doc/$id"
            page == null -> "home"
            page == "my-tasks" -> "tasks"
            page == "projects" && id != null -> "project/$id"
            page == "settings" && id == null -> "settings"
            page == "settings" && id == "notifications" -> "notifications"
            page == "settings" && id in SETTINGS -> "settings/$id"
            id == null && page in SCREENS -> page
            else -> null
        }
        return if (route != null) Screen.Native(ws, route) else Screen.Web(path)
    }

    fun resolve(link: String, appInstance: String?): LinkAction? {
        if (!link.startsWith("looty://w")) return null
        val tail = link.removePrefix("looty://w").substringBefore('#')
        if (tail.isNotEmpty() && tail[0] != '/' && tail[0] != '?') return null
        val all = params(tail.substringAfter('?', ""))
        val server = all.firstOrNull { it.startsWith("server=") }?.let { decode(it.removePrefix("server=")) ?: return null }
        val rest = all.filterNot { it.startsWith("server=") }
        val path = "/w" + tail.substringBefore('?') + if (rest.isEmpty()) "" else "?" + rest.joinToString("&")
        // Sans server : lien produit par l'app elle-même (notification, widget, raccourci).
        val app = appInstance?.let { origin(it) }
        if (server == null) return if (app == null) null else LinkAction.Open(path)
        val from = origin(server) ?: return null
        if (app == null) return LinkAction.Connect(from, path)
        if (from == app) return LinkAction.Open(path)
        return LinkAction.Foreign(from, app, from + path)
    }

    /** Texte partagé vers Looty qui n'est qu'un lien vers une page /w/… de l'instance de l'app : cette page. */
    fun sharedPage(text: String, appInstance: String?): String? {
        val app = appInstance?.let { origin(it) } ?: return null
        val s = text.trim()
        if (s.any { it.isWhitespace() }) return null
        val u = runCatching { URI(s) }.getOrNull() ?: return null
        if (origin(s) != app || !u.rawPath.orEmpty().startsWith("/w/")) return null
        return u.rawPath + (u.rawQuery?.let { "?$it" } ?: "")
    }

    /** « HTTPS://Looty.Org:443/ » → « https://looty.org » ; null si ce n'est pas une adresse http(s). */
    fun origin(url: String): String? {
        val u = runCatching { URI(url.trim()) }.getOrNull() ?: return null
        val scheme = u.scheme?.lowercase()?.takeIf { it == "https" || it == "http" } ?: return null
        val host = u.host?.lowercase() ?: return null
        val default = if (scheme == "https") 443 else 80
        return "$scheme://$host" + if (u.port > 0 && u.port != default) ":${u.port}" else ""
    }
}
