package io.github.untypequicode.looty

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Instances de l'app : migration de l'ancienne configuration, ajout, bascule, retrait (ADR 0017). */
class InstanceStoreTest {
    private class MapKv(val map: MutableMap<String, String> = mutableMapOf()) : KeyValue {
        override fun getString(key: String) = map[key]
        override fun putString(key: String, value: String) { map[key] = value }
        override fun remove(key: String) { map.remove(key) }
    }

    private val a = "https://looty.mon-asso.fr"
    private val b = "https://looty.ecole.fr"

    private fun store(kv: KeyValue = MapKv()): InstanceStore {
        var n = 0
        return InstanceStore(kv) { "id${++n}" }
    }

    @Test fun freshInstallHasNoInstance() {
        val s = store()
        assertTrue(s.all().isEmpty())
        assertNull(s.active())
    }

    @Test fun legacySingleServerBecomesTheDefaultInstance() {
        val kv = MapKv(mutableMapOf("server" to a, "workspace" to "ws1"))
        val s = store(kv)
        assertEquals(Instance("default", a), s.active())
        assertEquals(listOf(Instance("default", a)), s.all())
        // L'ancienne clé est conservée (retour arrière possible) et les autres réglages ne bougent pas.
        assertEquals(a, kv.map["server"])
        assertEquals("ws1", kv.map["workspace"])
    }

    @Test fun migrationIsDoneOnceAndSurvivesRestart() {
        val kv = MapKv(mutableMapOf("server" to a))
        store(kv).all()
        kv.map["server"] = "https://modifie.example" // ignoré : la liste fait foi une fois migrée
        assertEquals(Instance("default", a), store(kv).active())
    }

    @Test fun defaultInstanceKeepsHistoricStorageNames() {
        assertEquals("looty", InstanceStore.scoped("looty", "default"))
        assertEquals("looty.push", InstanceStore.scoped("looty.push", "default"))
        assertEquals("looty-poll.x9", InstanceStore.scoped("looty-poll", "x9"))
        // Dossier hors ligne historique : empreinte de l'adresse, inchangée donc aucune donnée à déplacer.
        assertEquals(Integer.toHexString(a.hashCode()), InstanceStore.dirKey(a))
    }

    @Test fun addingAnInstanceKeepsTheFirstAndActivatesTheNew() {
        val s = store(MapKv(mutableMapOf("server" to a)))
        val added = s.add(b)
        assertEquals(Instance("id1", b), added)
        assertEquals(listOf(Instance("default", a), added), s.all())
        assertEquals(added, s.active())
    }

    @Test fun addingAKnownAddressReusesItAndSwitchesToIt() {
        val s = store(MapKv(mutableMapOf("server" to a)))
        s.add(b)
        assertEquals(Instance("default", a), s.add(a))
        assertEquals(2, s.all().size)
        assertEquals(a, s.active()?.origin)
    }

    @Test fun switchingUpdatesTheActiveInstanceAndTheLegacyMirror() {
        val kv = MapKv(mutableMapOf("server" to a))
        val s = store(kv)
        val added = s.add(b)
        assertTrue(s.activate("default"))
        assertEquals(a, s.active()?.origin)
        assertEquals(a, kv.map["server"])
        assertFalse(s.activate("unknown"))
        assertEquals(a, s.active()?.origin)
        assertEquals(added.origin, s.get(added.id)?.origin)
    }

    @Test fun generatedIdsNeverCollide() {
        val kv = MapKv(mutableMapOf("server" to a))
        val s = InstanceStore(kv, sequenceOf("default", "dup", "dup", "new").iterator()::next)
        assertEquals("dup", s.add(b).id)
        assertEquals("new", s.add("https://troisieme.example").id)
    }

    @Test fun removingTheActiveInstanceActivatesAnother() {
        val s = store(MapKv(mutableMapOf("server" to a)))
        val added = s.add(b)
        assertEquals(Instance("default", a), s.remove(added.id))
        assertEquals(listOf(Instance("default", a)), s.all())
    }

    @Test fun removingAnInactiveInstanceKeepsTheActiveOne() {
        val s = store(MapKv(mutableMapOf("server" to a)))
        val added = s.add(b)
        s.activate("default")
        assertEquals(Instance("default", a), s.remove(added.id))
    }

    @Test fun removingTheLastInstanceLeavesNoInstance() {
        val kv = MapKv(mutableMapOf("server" to a))
        val s = store(kv)
        assertNull(s.remove("default"))
        assertTrue(s.all().isEmpty())
        // Pas de renaissance depuis l'ancienne clé.
        assertNull(store(kv).active())
    }

    @Test fun resolveNeverFallsBackSilentlyForAKnownButRemovedId() {
        val s = store(MapKv(mutableMapOf("server" to a)))
        val added = s.add(b)
        assertEquals(added, s.resolve(added.id))
        // Widget sans choix d'instance (posé avant) : l'instance d'origine.
        assertEquals(Instance("default", a), s.resolve(null))
        // get() est strict : un identifiant retiré ne désigne jamais une autre instance.
        s.remove(added.id)
        assertNull(s.get(added.id))
    }

    @Test fun corruptListFallsBackToTheLegacyServer() {
        val s = store(MapKv(mutableMapOf("instances" to "pas du json", "server" to a)))
        assertEquals(Instance("default", a), s.active())
    }
}
