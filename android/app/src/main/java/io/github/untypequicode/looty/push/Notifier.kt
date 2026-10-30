package io.github.untypequicode.looty.push

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationChannelGroup
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import io.github.untypequicode.looty.MainActivity
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Notification
import kotlinx.serialization.json.jsonPrimitive

/**
 * Affiche les notifications reçues (push) ou trouvées (vérification périodique), chacune dans
 * le canal de son type : dans les réglages d'Android, chaque type a son importance, son son,
 * sa vibration et son affichage sur l'écran verrouillé.
 */
object Notifier {
    /** Canal historique (avant les canaux par type) : gardé pour « Autres », avec ses réglages. */
    const val CHANNEL_ACTIVITY = "activity"

    /** Un type de notification : canal Android, groupe, importance par défaut, libellés. */
    enum class Kind(val channel: String, val group: String, val importance: Int, val title: Int, val hint: Int) {
        ASSIGNED("assigned", GROUP_WORK, NotificationManager.IMPORTANCE_HIGH, R.string.ch_assigned, R.string.ch_assigned_hint),
        MENTION("mentions", GROUP_WORK, NotificationManager.IMPORTANCE_HIGH, R.string.ch_mentions, R.string.ch_mentions_hint),
        COMMENT("comments", GROUP_WORK, NotificationManager.IMPORTANCE_DEFAULT, R.string.ch_comments, R.string.ch_comments_hint),
        DUE("due", GROUP_WORK, NotificationManager.IMPORTANCE_HIGH, R.string.ch_due, R.string.ch_due_hint),
        COMPLETED("completed", GROUP_WORK, NotificationManager.IMPORTANCE_LOW, R.string.ch_completed, R.string.ch_completed_hint),
        MAIL("mail", GROUP_MESSAGES, NotificationManager.IMPORTANCE_DEFAULT, R.string.ch_mail, R.string.ch_mail_hint),
        MOODLE("moodle", GROUP_MESSAGES, NotificationManager.IMPORTANCE_DEFAULT, R.string.ch_moodle, R.string.ch_moodle_hint),
        EVENTS("events", GROUP_MESSAGES, NotificationManager.IMPORTANCE_DEFAULT, R.string.ch_events, R.string.ch_events_hint),
        UPDATES("updates", GROUP_APP, NotificationManager.IMPORTANCE_LOW, R.string.ch_updates, R.string.ch_updates_hint),
        OTHER(CHANNEL_ACTIVITY, GROUP_APP, NotificationManager.IMPORTANCE_DEFAULT, R.string.ch_other, R.string.ch_other_hint);

        companion object {
            /** Type d'après le serveur : `kind` du push (task_assigned…) ou de la boîte de réception (assigned…). */
            fun of(kind: String?): Kind = when (kind) {
                "task_assigned", "assigned" -> ASSIGNED
                "task_mention", "doc_mention", "mention" -> MENTION
                "task_comment", "comment" -> COMMENT
                "task_due", "due_soon" -> DUE
                "task_completed", "completed" -> COMPLETED
                "mail_received", "mail" -> MAIL
                "moodle_received", "moodle" -> MOODLE
                "event_tag" -> EVENTS
                "update" -> UPDATES
                else -> OTHER
            }
        }
    }

    private const val GROUP_WORK = "work"
    private const val GROUP_MESSAGES = "messages"
    private const val GROUP_APP = "app"

    fun ensureChannels(ctx: Context) {
        if (Build.VERSION.SDK_INT < 26) return
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannelGroups(
            listOf(
                NotificationChannelGroup(GROUP_WORK, ctx.getString(R.string.chg_work)),
                NotificationChannelGroup(GROUP_MESSAGES, ctx.getString(R.string.chg_messages)),
                NotificationChannelGroup(GROUP_APP, ctx.getString(R.string.chg_app)),
            ),
        )
        // Création idempotente : les réglages choisis par la personne ne sont jamais écrasés.
        nm.createNotificationChannels(
            Kind.entries.map { k ->
                NotificationChannel(k.channel, ctx.getString(k.title), k.importance).apply {
                    description = ctx.getString(k.hint)
                    group = k.group
                    if (k == Kind.MENTION || k == Kind.ASSIGNED || k == Kind.DUE) enableVibration(true)
                }
            },
        )
    }

    /** Canal encore actif (désactivé dans les réglages d'Android : rien n'est affiché). */
    fun channelEnabled(ctx: Context, kind: Kind): Boolean {
        if (Build.VERSION.SDK_INT < 26) return true
        val ch = ctx.getSystemService(NotificationManager::class.java).getNotificationChannel(kind.channel) ?: return true
        return ch.importance != NotificationManager.IMPORTANCE_NONE
    }

    fun permitted(ctx: Context): Boolean =
        (Build.VERSION.SDK_INT < 33 || ctx.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) &&
            NotificationManagerCompat.from(ctx).areNotificationsEnabled()

    /** Chemin de l'application (« /w/<espace>/… ») → lien ouvert au clic. */
    private fun intentFor(ctx: Context, path: String?): Intent {
        val intent = Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        if (path != null && path.startsWith("/w/")) {
            // « /w/<espace>/… » → looty://w/<espace>/… (même lien que « Ouvrir dans l'application »).
            intent.action = Intent.ACTION_VIEW
            intent.data = Uri.parse("looty:/" + path)
        }
        return intent
    }

    /** Une notification : `tag` identifie l'évènement (une même notification ne s'affiche qu'une fois). */
    fun show(ctx: Context, title: String, body: String, path: String?, tag: String?, kind: Kind = Kind.OTHER) {
        if (!permitted(ctx)) return
        ensureChannels(ctx)
        val id = (tag ?: "$title|$body").hashCode()
        val pi = PendingIntent.getActivity(ctx, id, intentFor(ctx, path), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val n = NotificationCompat.Builder(ctx, kind.channel)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setContentIntent(pi)
            .setAutoCancel(true)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .setGroup("looty-${kind.group}")
            .build()
        runCatching { NotificationManagerCompat.from(ctx).notify(id, n) }
    }

    /** Ligne de texte d'une notification de la boîte de réception (même texte que l'écran). */
    fun line(ctx: Context, n: Notification): String {
        val who = n.actor_name ?: "?"
        return when (n.kind) {
            "mention" -> ctx.getString(R.string.n_mention, who)
            "assigned" -> ctx.getString(R.string.n_assigned, who)
            "comment" -> ctx.getString(R.string.n_comment, who)
            "due_soon" -> ctx.getString(R.string.n_due_soon)
            "completed" -> ctx.getString(R.string.n_completed, who)
            "mail" -> ctx.getString(R.string.n_mail)
            "moodle" -> ctx.getString(R.string.n_moodle)
            "event_tag" -> ctx.getString(R.string.n_event_tag, who)
            else -> ctx.getString(R.string.n_other)
        }
    }

    /** Chemin de l'application ouvert par une notification de la boîte de réception. */
    fun path(n: Notification): String {
        val ws = n.workspace_id
        val (kind, id) = n.target.split(":", limit = 2).let { it[0] to it.getOrElse(1) { "" } }
        return when (kind) {
            "task" -> n.data["board_id"]?.jsonPrimitive?.content?.let { "/w/$ws/board/$it?task=$id" } ?: "/w/$ws/task/$id"
            "page" -> "/w/$ws/doc/$id"
            "board" -> "/w/$ws/board/$id"
            "mail" -> "/w/$ws/mail?account=$id"
            else -> "/w/$ws/inbox"
        }
    }
}
