package io.github.untypequicode.looty.widget

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit

/** Rafraîchit le widget toutes les 30 minutes (quand il est posé sur l'écran d'accueil). */
class WidgetWorker(ctx: Context, params: WorkerParameters) : CoroutineWorker(ctx, params) {
    override suspend fun doWork(): Result {
        Widgets.updateAll(applicationContext)
        return Result.success()
    }

    companion object {
        fun schedule(ctx: Context) {
            WorkManager.getInstance(ctx).enqueueUniquePeriodicWork("looty-widget", ExistingPeriodicWorkPolicy.KEEP, PeriodicWorkRequestBuilder<WidgetWorker>(30, TimeUnit.MINUTES).build())
        }
    }
}
