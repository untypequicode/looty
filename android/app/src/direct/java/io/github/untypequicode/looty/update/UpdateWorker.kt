package io.github.untypequicode.looty.update

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import io.github.untypequicode.looty.R
import io.github.untypequicode.looty.api.Api
import io.github.untypequicode.looty.push.Notifier
import java.util.concurrent.TimeUnit

/** Vérifie chaque jour s'il existe une nouvelle version et le signale par une notification (une seule par version). */
class UpdateWorker(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {
    override suspend fun doWork(): Result {
        val prefs = applicationContext.getSharedPreferences("looty", 0)
        if (!prefs.getBoolean("autoUpdateCheck", true)) return Result.success()
        val lang = java.util.Locale.getDefault().language
        val update = runCatching { Updater.check(applicationContext, Api(applicationContext), lang) }.getOrElse { return Result.retry() }
        prefs.edit().putString("updateAvailable", update?.version).putLong("updateChecked", System.currentTimeMillis()).apply()
        if (update != null && prefs.getString("updateNotified", null) != update.version) {
            prefs.edit().putString("updateNotified", update.version).apply()
            Notifier.show(applicationContext, applicationContext.getString(R.string.update_available_title), applicationContext.getString(R.string.update_available_body, update.version), "/app/updates", "update-" + update.version, Notifier.Kind.UPDATES)
        }
        return Result.success()
    }

    companion object {
        fun schedule(ctx: Context) {
            val wm = WorkManager.getInstance(ctx)
            if (!ctx.getSharedPreferences("looty", 0).getBoolean("autoUpdateCheck", true)) { wm.cancelUniqueWork("looty-update"); return }
            val req = PeriodicWorkRequestBuilder<UpdateWorker>(24, TimeUnit.HOURS).setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()).build()
            wm.enqueueUniquePeriodicWork("looty-update", ExistingPeriodicWorkPolicy.KEEP, req)
        }
    }
}
