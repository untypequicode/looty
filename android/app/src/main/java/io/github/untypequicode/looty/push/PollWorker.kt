package io.github.untypequicode.looty.push

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import io.github.untypequicode.looty.Config
import io.github.untypequicode.looty.api.Api
import io.github.untypequicode.looty.api.ApiException
import io.github.untypequicode.looty.api.Notification
import io.github.untypequicode.looty.api.Workspaces
import java.time.Instant

/**
 * Sans application de distribution UnifiedPush : toutes les ~15 minutes, les notifications
 * non lues apparues depuis la dernière vérification sont affichées.
 */
class PollWorker(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {
    override suspend fun doWork(): Result {
        val ctx = applicationContext
        if (PushStore.mode(ctx) != PushMode.POLLING || Config.server(ctx) == null) return Result.success()
        val api = Api(ctx)
        val since = PushStore.lastPoll(ctx)
        val shown = PushStore.shown(ctx).toMutableSet()
        var latest = since
        return try {
            val workspaces = api.get<Workspaces>("/api/workspaces").workspaces
            var count = 0
            for (w in workspaces) {
                val list = api.get<List<Notification>>("/api/notifications?workspace_id=${w.id}")
                for (n in list.sortedBy { it.created_at }) {
                    val at = runCatching { Instant.parse(n.created_at).toEpochMilli() }.getOrDefault(0L)
                    latest = maxOf(latest, at)
                    if (n.read_at != null || at <= since || !shown.add(n.id) || count >= 8) continue
                    count++
                    Notifier.show(ctx, w.name, Notifier.line(ctx, n), Notifier.path(n), n.id, Notifier.Kind.of(n.kind))
                }
            }
            PushStore.setLastPoll(ctx, maxOf(latest, since))
            // Mémoire bornée : seuls les identifiants récents servent à éviter les doublons.
            PushStore.setShown(ctx, shown.toList().takeLast(200).toSet())
            Result.success()
        } catch (e: ApiException) {
            // Session expirée : rien à faire avant la prochaine connexion.
            if (e.status == 401 || e.status == 403) Result.success() else Result.retry()
        } catch (_: Exception) {
            Result.retry()
        }
    }
}
