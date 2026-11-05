package io.github.untypequicode.looty

import io.github.untypequicode.looty.InstanceLinks.LinkAction
import io.github.untypequicode.looty.InstanceLinks.Screen
import org.junit.Assert.assertEquals
import org.junit.Test

/** Liens looty://w/… reçus : n'ouvrir que ceux de l'instance de l'app (ADR 0004). */
class InstanceLinksTest {
    private val app = "https://looty.mon-asso.fr"

    @Test fun linkFromTheAppInstanceOpensThePageWithoutServer() {
        assertEquals(
            LinkAction.Open("/w/ws1/board/b1?task=t1"),
            InstanceLinks.resolve("looty://w/ws1/board/b1?task=t1&server=https%3A%2F%2Flooty.mon-asso.fr", app),
        )
    }

    @Test fun linkFromAnotherInstanceIsOfferedInTheBrowser() {
        assertEquals(
            LinkAction.Foreign("https://autre.fr", app, "https://autre.fr/w/ws1/doc/p1?comment=c1"),
            InstanceLinks.resolve("looty://w/ws1/doc/p1?comment=c1&server=https%3A%2F%2Fautre.fr", app),
        )
    }

    @Test fun linkWithoutServerComesFromTheAppAndOpens() {
        // Notifications, widgets, raccourcis.
        assertEquals(LinkAction.Open("/w/ws1/task/t1"), InstanceLinks.resolve("looty://w/ws1/task/t1", app))
    }

    @Test fun appWithoutInstanceOffersToConnectToTheLinkInstance() {
        assertEquals(
            LinkAction.Connect("https://autre.fr", "/w/ws1/announcements"),
            InstanceLinks.resolve("looty://w/ws1/announcements?server=https%3A%2F%2FAutre.fr", null),
        )
    }

    @Test fun invalidOrMissingServerWithoutInstanceIsIgnored() {
        assertEquals(null, InstanceLinks.resolve("looty://w/ws1?server=javascript%3Aalert(1)", app))
        assertEquals(null, InstanceLinks.resolve("looty://w/ws1", null))
        assertEquals(null, InstanceLinks.resolve("looty://sso/ws1", app))
    }

    @Test fun sameInstanceWrittenDifferentlyStillOpens() {
        // Schéma et hôte en majuscules, port par défaut explicite, barre finale.
        assertEquals(LinkAction.Open("/w/ws1/calendar"), InstanceLinks.resolve("looty://w/ws1/calendar?server=HTTPS%3A%2F%2FLooty.Mon-Asso.FR%3A443%2F", app))
        assertEquals(LinkAction.Open("/w/ws1"), InstanceLinks.resolve("looty://w/ws1?server=https%3A%2F%2Flooty.mon-asso.fr", "https://looty.mon-asso.fr:443"))
        assertEquals(LinkAction.Open("/w/ws1"), InstanceLinks.resolve("looty://w/ws1?server=http%3A%2F%2F10.0.2.2%3A80", "http://10.0.2.2"))
    }

    @Test fun httpAndHttpsAreDifferentInstances() {
        assertEquals(
            LinkAction.Foreign("http://looty.mon-asso.fr", app, "http://looty.mon-asso.fr/w/ws1"),
            InstanceLinks.resolve("looty://w/ws1?server=http%3A%2F%2Flooty.mon-asso.fr", app),
        )
    }

    @Test fun taskLinkOpensTheNativeTaskScreen() {
        assertEquals(Screen.Native("ws1", "task/t1"), InstanceLinks.screen("/w/ws1/task/t1"))
    }

    @Test fun boardAndDocumentLinksOpenTheirNativeScreens() {
        assertEquals(Screen.Native("ws1", "task/t1"), InstanceLinks.screen("/w/ws1/board/b1?task=t1"))
        assertEquals(Screen.Native("ws1", "board/b1"), InstanceLinks.screen("/w/ws1/board/b1"))
        assertEquals(Screen.Native("ws1", "doc/p1/comments"), InstanceLinks.screen("/w/ws1/doc/p1?comment=c1"))
        assertEquals(Screen.Native("ws1", "doc/p1/history"), InstanceLinks.screen("/w/ws1/doc/p1?history=1"))
        assertEquals(Screen.Native("ws1", "doc/p1"), InstanceLinks.screen("/w/ws1/doc/p1"))
        assertEquals(Screen.Native("ws1", "doc/p1"), InstanceLinks.screen("/w/ws1/read/p1"))
    }

    @Test fun workspaceScreensOpenNatively() {
        val expected = mapOf(
            "/w/ws1" to "home",
            "/w/ws1/" to "home",
            "/w/ws1/my-tasks" to "tasks",
            "/w/ws1/docs" to "docs",
            "/w/ws1/inbox" to "inbox",
            "/w/ws1/calendar" to "calendar",
            "/w/ws1/announcements" to "announcements",
            "/w/ws1/drive" to "drive",
            "/w/ws1/moodle" to "moodle",
            "/w/ws1/activity" to "activity",
            "/w/ws1/trash" to "trash",
            "/w/ws1/graph" to "graph",
            "/w/ws1/projects" to "projects",
            "/w/ws1/projects/pr1" to "project/pr1",
            "/w/ws1/mail" to "mail",
            "/w/ws1/mail?account=a1" to "mail",
        )
        expected.forEach { (path, route) -> assertEquals(path, Screen.Native("ws1", route), InstanceLinks.screen(path)) }
    }

    @Test fun settingsSectionsOpenNatively() {
        assertEquals(Screen.Native("ws1", "settings"), InstanceLinks.screen("/w/ws1/settings"))
        assertEquals(Screen.Native("ws1", "notifications"), InstanceLinks.screen("/w/ws1/settings/notifications"))
        assertEquals(Screen.Native("ws1", "settings/calendars"), InstanceLinks.screen("/w/ws1/settings/calendars"))
        assertEquals(Screen.Native("ws1", "settings/members"), InstanceLinks.screen("/w/ws1/settings/members"))
    }

    @Test fun pagesWithoutNativeScreenOpenTheWebScreen() {
        listOf("/w/ws1/canvas", "/w/ws1/whats-new", "/w/ws1/settings/instance", "/w/ws1/inbox/extra", "/login").forEach {
            assertEquals(it, Screen.Web(it), InstanceLinks.screen(it))
        }
    }

    @Test fun sharedLinkOfTheAppInstanceOpensThePage() {
        assertEquals("/w/ws1/board/b1?task=t1", InstanceLinks.sharedPage("https://Looty.mon-asso.fr/w/ws1/board/b1?task=t1#x", app))
        assertEquals("/w/ws1/doc/p1", InstanceLinks.sharedPage("  https://looty.mon-asso.fr:443/w/ws1/doc/p1\n", app))
    }

    @Test fun sharedTextThatIsNotAPageOfTheAppInstanceIsKept() {
        assertEquals(null, InstanceLinks.sharedPage("https://autre.fr/w/ws1/doc/p1", app))
        assertEquals(null, InstanceLinks.sharedPage("https://looty.mon-asso.fr/login", app))
        assertEquals(null, InstanceLinks.sharedPage("À lire : https://looty.mon-asso.fr/w/ws1/doc/p1", app))
        assertEquals(null, InstanceLinks.sharedPage("https://looty.mon-asso.fr/w/ws1/doc/p1", null))
    }

    @Test fun wwwOrAnotherPortIsAnotherInstance() {
        assertEquals(
            LinkAction.Foreign("https://www.looty.mon-asso.fr", app, "https://www.looty.mon-asso.fr/w/ws1"),
            InstanceLinks.resolve("looty://w/ws1?server=https%3A%2F%2Fwww.looty.mon-asso.fr", app),
        )
        assertEquals(
            LinkAction.Foreign("https://looty.mon-asso.fr:8443", app, "https://looty.mon-asso.fr:8443/w/ws1"),
            InstanceLinks.resolve("looty://w/ws1?server=https%3A%2F%2Flooty.mon-asso.fr%3A8443", app),
        )
    }
}
