package io.github.untypequicode.looty.call

import io.github.untypequicode.looty.call.CallPolicy.Need
import io.github.untypequicode.looty.call.CallPolicy.Route
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Micro et caméra : seulement pour l'instance de l'application (ADR 0019, ADR 0017). */
class CallPolicyTest {
    private val server = "https://looty.mon-asso.fr"
    private val audio = arrayOf(CallPolicy.RESOURCE_AUDIO)
    private val both = arrayOf(CallPolicy.RESOURCE_AUDIO, CallPolicy.RESOURCE_VIDEO)

    @Test fun originIgnoresCaseTrailingSlashAndDefaultPort() {
        assertEquals("https://looty.fr", CallPolicy.originOf("https://Looty.FR:443/"))
        assertEquals("http://10.0.2.2:3000", CallPolicy.originOf("http://10.0.2.2:3000/w/x"))
        assertNull(CallPolicy.originOf("javascript:alert(1)"))
        assertNull(CallPolicy.originOf("not a url"))
    }

    @Test fun micIsGrantedToTheAppInstanceOnly() {
        assertEquals(Need(audio = true, video = false), CallPolicy.need(server, "https://looty.mon-asso.fr/", audio))
        assertNull(CallPolicy.need(server, "https://autre.fr", audio))
        assertNull(CallPolicy.need(server, "https://looty.mon-asso.fr.evil.fr", audio))
        assertNull(CallPolicy.need(server, "http://looty.mon-asso.fr", audio))
        assertNull(CallPolicy.need(server, "https://looty.mon-asso.fr:8443", audio))
    }

    @Test fun otherResourcesAreNotGranted() {
        assertNull(CallPolicy.need(server, server, arrayOf("android.webkit.resource.MIDI_SYSEX")))
        assertNull(CallPolicy.need(server, server, arrayOf("android.webkit.resource.PROTECTED_MEDIA_ID")))
        // Mélange : seuls micro et caméra sont accordés.
        val n = CallPolicy.need(server, server, arrayOf("android.webkit.resource.MIDI_SYSEX", CallPolicy.RESOURCE_VIDEO))!!
        assertArrayEquals(arrayOf(CallPolicy.RESOURCE_VIDEO), CallPolicy.grant(n, micGranted = false, cameraGranted = true))
    }

    @Test fun missingSystemPermissionsAreRequestedOnlyWhenNeeded() {
        val n = CallPolicy.need(server, server, both)!!
        assertEquals(listOf("android.permission.RECORD_AUDIO", "android.permission.CAMERA"), CallPolicy.missing(n, false, false))
        assertEquals(listOf("android.permission.CAMERA"), CallPolicy.missing(n, true, false))
        assertTrue(CallPolicy.missing(n, true, true).isEmpty())
        assertTrue(CallPolicy.missing(Need(audio = true, video = false), true, false).isEmpty())
    }

    @Test fun refusedPermissionGrantsNothing() {
        val n = CallPolicy.need(server, server, both)!!
        assertEquals(0, CallPolicy.grant(n, micGranted = false, cameraGranted = false).size)
        assertArrayEquals(audio, CallPolicy.grant(n, micGranted = true, cameraGranted = false))
    }

    @Test fun headsetWinsThenSpeakerOrEarpiece() {
        assertEquals(Route.HEADSET, CallPolicy.route(headsetAvailable = true, speaker = true))
        assertEquals(Route.SPEAKER, CallPolicy.route(false, true))
        assertEquals(Route.EARPIECE, CallPolicy.route(false, false))
        assertTrue(CallPolicy.defaultSpeaker())
        assertFalse(CallPolicy.route(false, CallPolicy.defaultSpeaker()) == Route.EARPIECE)
    }
}
