package io.github.untypequicode.looty.call

import java.net.URI

/**
 * Règles des appels (ADR 0019), sans accès à Android : testables sur la JVM.
 * - le micro et la caméra ne sont accordés qu'à la page de l'instance de l'application ;
 * - rien d'autre que ces deux ressources (pas de MIDI, de médias protégés…) ;
 * - choix de la sortie audio.
 */
object CallPolicy {
    // Valeurs de android.webkit.PermissionRequest (inscrites ici pour rester testable sans Android).
    const val RESOURCE_AUDIO = "android.webkit.resource.AUDIO_CAPTURE"
    const val RESOURCE_VIDEO = "android.webkit.resource.VIDEO_CAPTURE"

    /** « scheme://hôte[:port] » en minuscules, ou null ; le port par défaut est retiré. */
    fun originOf(url: String): String? {
        val u = runCatching { URI(url.trim()) }.getOrNull() ?: return null
        val scheme = u.scheme?.lowercase() ?: return null
        val host = u.host?.lowercase() ?: return null
        if (scheme != "https" && scheme != "http") return null
        val port = when {
            u.port < 0 -> ""
            scheme == "https" && u.port == 443 -> ""
            scheme == "http" && u.port == 80 -> ""
            else -> ":${u.port}"
        }
        return "$scheme://$host$port"
    }

    fun sameOrigin(server: String, origin: String): Boolean {
        val a = originOf(server) ?: return false
        return a == originOf(origin)
    }

    /** Ressources demandées par la page. */
    data class Need(val audio: Boolean, val video: Boolean)

    /** Ressources à accorder parmi celles demandées ; null : refuser (autre origine, ou ni micro ni caméra). */
    fun need(server: String, origin: String, resources: Array<String>): Need? {
        if (!sameOrigin(server, origin)) return null
        val need = Need(RESOURCE_AUDIO in resources, RESOURCE_VIDEO in resources)
        return if (need.audio || need.video) need else null
    }

    /** Ressources WebView à accorder une fois les permissions du système connues. */
    fun grant(need: Need, micGranted: Boolean, cameraGranted: Boolean): Array<String> = buildList {
        if (need.audio && micGranted) add(RESOURCE_AUDIO)
        if (need.video && cameraGranted) add(RESOURCE_VIDEO)
    }.toTypedArray()

    /** Permissions du système à demander (celles qui manquent). */
    fun missing(need: Need, micGranted: Boolean, cameraGranted: Boolean): List<String> = buildList {
        if (need.audio && !micGranted) add("android.permission.RECORD_AUDIO")
        if (need.video && !cameraGranted) add("android.permission.CAMERA")
    }

    enum class Route { HEADSET, SPEAKER, EARPIECE }

    /** Casque (filaire, USB ou Bluetooth) si branché ; sinon haut-parleur ou écouteur selon le choix. */
    fun route(headsetAvailable: Boolean, speaker: Boolean): Route = when {
        headsetAvailable -> Route.HEADSET
        speaker -> Route.SPEAKER
        else -> Route.EARPIECE
    }

    /** Haut-parleur par défaut : on parle en travaillant, le téléphone n'est pas collé à l'oreille. */
    fun defaultSpeaker(): Boolean = true
}
