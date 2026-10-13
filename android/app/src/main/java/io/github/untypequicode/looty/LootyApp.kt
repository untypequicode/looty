package io.github.untypequicode.looty

import android.app.Application

class LootyApp : Application() {
    override fun onCreate() {
        super.onCreate()
        io.github.untypequicode.looty.push.Notifier.ensureChannels(this)
        io.github.untypequicode.looty.push.Push.restore(this)
        io.github.untypequicode.looty.widget.WidgetWorker.schedule(this)
        io.github.untypequicode.looty.update.UpdateWorker.schedule(this)
    }
}
