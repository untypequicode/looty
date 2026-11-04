package io.github.untypequicode.looty.ui

import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.AnnouncementList
import io.github.untypequicode.looty.api.Task
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Annonces, sondages et tâches telles que le serveur les renvoie. */
class AnnouncementModelsTest {
    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false; coerceInputValues = true }

    @Test fun targetedAnnouncementWithAnonymousPoll() {
        val l = json.decodeFromString<AnnouncementList>(
            """{"announcements":[{"id":"a1","workspace_id":"w","author_id":"u1","author_name":"Alice","title":"Salle du TD2","body":"<p>B12</p>",
               "pinned":true,"expires_at":null,"created_at":"2026-11-02T10:00:00Z","updated_at":"2026-11-02T10:00:00Z","read":false,"expired":false,
               "targeted":true,"for_me":false,"audience":{"groups":["g1"],"users":[]},
               "poll":{"question":"OK ?","multiple":false,"anonymous":true,"results_after_vote":true,"closes_at":null,"closed":false,"results_visible":false,
                       "voter_count":3,"my_votes":[],"options":[{"id":"o1","label":"Oui","votes":null,"voters":null},{"id":"o2","label":"Non","votes":null,"voters":null}]}}],
               "unread":0,"can_manage":true}""",
        )
        val a = l.announcements.single()
        assertTrue(a.targeted && a.pinned && l.can_manage)
        assertFalse(a.for_me)
        assertEquals(listOf("g1"), a.audience?.groups)
        val p = a.poll!!
        assertFalse(p.results_visible)
        assertNull(p.options.first().votes)
        assertEquals(3, p.voter_count)
    }

    @Test fun announcementForEveryoneHasNoAudience() {
        val a = json.decodeFromString<AnnouncementList>("""{"announcements":[{"id":"a","title":"T","created_at":"2026-11-02T10:00:00Z","audience":null,"poll":null}]}""").announcements.single()
        assertTrue(a.for_me)
        assertNull(a.audience)
    }

    @Test fun taskRepeatPersonalAndGroups() {
        val t = json.decodeFromString<Task>("""{"id":"t","column_id":"c","title":"TP","repeat":"biweekly","private_to":"u1","groups":["g1"],"keywords":["inf301"]}""")
        assertEquals("biweekly", t.repeat)
        assertEquals("u1", t.private_to)
        assertEquals(listOf("g1"), t.groups)
        assertEquals(R.string.repeat_biweekly, repeatLabel(t.repeat))
        assertEquals(R.string.repeat_none, repeatLabel(null))
    }
}
