package io.github.untypequicode.looty.update

import android.content.Context
import androidx.work.WorkManager

/** Variante « fdroid » : pas de vérification quotidienne. Retire celle d'une version « direct » installée avant. */
object UpdateWorker {
    fun schedule(ctx: Context) {
        WorkManager.getInstance(ctx).cancelUniqueWork("looty-update")
    }
}
