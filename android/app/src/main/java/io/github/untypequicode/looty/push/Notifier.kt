package io.github.untypequicode.looty.push

import android.Manifest
import android.app.NotificationChannel
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

/** Affiche les notifications reçues (push) ou trouvées (vérification périodique). */
object Notifier {
    const val CHANNEL_ACTIVITY = "activity"

    fun ensureChannels(ctx: Context) {
        if (Build.VERSION.SDK_INT < 26) return
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_ACTIVITY, ctx.getString(R.string.channel_activity), NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = ctx.getString(R.string.channel_activity_hint)
            },
        )
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
    fun show(ctx: Context, title: String, body: String, path: String?, tag: String?) {
        if (!permitted(ctx)) return
        ensureChannels(ctx)
        val id = (tag ?: "$title|$body").hashCode()
        val pi = PendingIntent.getActivity(ctx, id, intentFor(ctx, path), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val n = NotificationCompat.Builder(ctx, CHANNEL_ACTIVITY)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(NotificationCompat.BigTextStyle().bigText(body))
            .setContentIntent(pi)
            .setAutoCancel(true)
            .setCategory(NotificationCompat.CATEGORY_MESSAGE)
            .setGroup("looty")
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
