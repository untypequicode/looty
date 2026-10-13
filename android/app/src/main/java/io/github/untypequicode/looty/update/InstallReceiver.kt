package io.github.untypequicode.looty.update

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller

/** Résultat de l'installateur du système : demande de confirmation, succès ou échec (gardé pour l'écran « Mises à jour »). */
class InstallReceiver : BroadcastReceiver() {
    override fun onReceive(ctx: Context, intent: Intent) {
        val prefs = ctx.getSharedPreferences("looty", 0)
        when (val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                @Suppress("DEPRECATION")
                val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
                confirm?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)?.let { runCatching { ctx.startActivity(it) } }
                prefs.edit().putString("updateStatus", "confirm").apply()
            }
            PackageInstaller.STATUS_SUCCESS -> prefs.edit().putString("updateStatus", "done").apply()
            else -> prefs.edit().putString("updateStatus", "failed:" + status + ":" + (intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: "")).apply()
        }
    }
}
