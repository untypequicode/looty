package io.github.untypequicode.looty

import android.content.Context
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/** Une instance Looty connue de l'app (ADR 0017). [id] nomme son espace de stockage sur l'appareil. */
@Serializable
data class Instance(val id: String, val origin: String)

/** Stockage clé/valeur minimal : SharedPreferences sur l'appareil, une table en mémoire dans les tests. */
interface KeyValue {
    fun getString(key: String): String?
    fun putString(key: String, value: String)
    fun remove(key: String)
}

/**
 * Liste des instances et instance active, avec la migration de l'ancienne configuration (une seule
 * instance, clé « server »). Sans accès à Android : testable sur la JVM.
 */
class InstanceStore(private val kv: KeyValue, private val newId: () -> String = { java.util.UUID.randomUUID().toString().take(8) }) {
    companion object {
        /** Identifiant de l'instance issue de l'ancienne configuration : ses données restent à leur place. */
        const val DEFAULT = "default"
        private const val LIST = "instances"
        private const val ACTIVE = "activeInstance"
        private const val LEGACY = "server"

        /** Nom d'un fichier de réglages (ou d'un travail de fond) propre à une instance : inchangé pour « default ». */
        fun scoped(base: String, id: String) = if (id == DEFAULT) base else "$base.$id"

        /** Dossier de données d'une instance sous filesDir/offline et webcache (nom historique : empreinte de l'adresse). */
        fun dirKey(origin: String) = Integer.toHexString(origin.hashCode())
    }

    private fun decode(raw: String?): List<Instance>? = raw?.let { runCatching { Json.decodeFromString<List<Instance>>(it) }.getOrNull() }

    private fun encode(list: List<Instance>) = Json.encodeToString(list)

    /** Instances connues. L'ancienne configuration devient l'instance « default » (migration, une seule fois). */
    fun all(): List<Instance> {
        decode(kv.getString(LIST))?.let { return it }
        val legacy = kv.getString(LEGACY) ?: return emptyList()
        val list = listOf(Instance(DEFAULT, legacy))
        kv.putString(LIST, encode(list))
        kv.putString(ACTIVE, DEFAULT)
        return list
    }

    fun active(): Instance? {
        val list = all()
        val id = kv.getString(ACTIVE)
        return list.firstOrNull { it.id == id } ?: list.firstOrNull()
    }

    fun get(id: String?): Instance? = id?.let { i -> all().firstOrNull { it.id == i } }

    fun byOrigin(origin: String): Instance? = all().firstOrNull { it.origin == origin }

    /** Ajoute (ou retrouve) l'instance d'adresse [origin] et la rend active. */
    fun add(origin: String): Instance {
        val existing = byOrigin(origin)
        val inst = existing ?: Instance(freshId(), origin).also { kv.putString(LIST, encode(all() + it)) }
        activate(inst.id)
        return inst
    }

    private fun freshId(): String {
        val taken = all().map { it.id }.toSet() + DEFAULT
        return generateSequence { newId() }.first { it !in taken }
    }

    fun activate(id: String): Boolean {
        if (get(id) == null) return false
        kv.putString(ACTIVE, id)
        // Miroir de l'ancienne clé : une version précédente de l'app retrouve l'instance active.
        kv.putString(LEGACY, get(id)!!.origin)
        return true
    }

    /** Retire l'instance ; si c'était l'active, la première restante le devient. Renvoie la nouvelle instance active. */
    fun remove(id: String): Instance? {
        val rest = all().filterNot { it.id == id }
        kv.putString(LIST, encode(rest))
        if (kv.getString(ACTIVE) == id) {
            val next = rest.firstOrNull()
            if (next == null) { kv.remove(ACTIVE); kv.remove(LEGACY) } else activate(next.id)
        }
        return active()
    }

    /** Instance d'un widget ou d'une tâche de fond : [id] s'il existe, sinon la plus ancienne (ancien widget sans instance). */
    fun resolve(id: String?): Instance? = get(id) ?: get(DEFAULT) ?: active()
}

/** Accès aux instances depuis Android (réglages « looty », partagés par tout le processus). */
object Instances {
    private fun kv(ctx: Context) = object : KeyValue {
        private val p = ctx.applicationContext.getSharedPreferences("looty", Context.MODE_PRIVATE)
        override fun getString(key: String): String? = p.getString(key, null)
        override fun putString(key: String, value: String) { p.edit().putString(key, value).commit() }
        override fun remove(key: String) { p.edit().remove(key).commit() }
    }

    @Synchronized fun store(ctx: Context) = InstanceStore(kv(ctx))

    fun all(ctx: Context) = store(ctx).all()
    fun active(ctx: Context) = store(ctx).active()
    fun activeId(ctx: Context): String = active(ctx)?.id ?: InstanceStore.DEFAULT
    /** Adresse de l'instance [id] ; null si elle n'existe plus (jamais une autre instance par défaut). */
    fun origin(ctx: Context, id: String?): String? = store(ctx).get(id)?.origin

    /** Réglages de l'instance [id] (workspace choisi, vues…) ; « looty » pour l'instance d'origine. */
    fun prefs(ctx: Context, id: String) = ctx.applicationContext.getSharedPreferences(InstanceStore.scoped("looty", id), Context.MODE_PRIVATE)
}

/** Passe à l'instance [id] : l'application redémarre sur un état neuf (rien ne reste de l'instance précédente en mémoire). */
fun Instances.switchTo(ctx: Context, id: String, link: String? = null) {
    if (!store(ctx).activate(id)) return
    val intent = android.content.Intent(ctx, MainActivity::class.java).addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK or android.content.Intent.FLAG_ACTIVITY_CLEAR_TASK)
    if (link != null) intent.setData(android.net.Uri.parse(link))
    ctx.startActivity(intent)
    (ctx as? android.app.Activity)?.finish()
}

/** Instance d'un widget : celle choisie à son ajout ; un ancien widget (sans choix) garde l'instance d'origine. Null si elle a été retirée. */
fun Instances.forWidget(ctx: Context, id: String?): Instance? = if (id == null) store(ctx).resolve(null) else store(ctx).get(id)
