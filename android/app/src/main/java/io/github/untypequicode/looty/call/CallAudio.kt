package io.github.untypequicode.looty.call

import android.content.Context
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build

/**
 * Routage audio pendant un appel : mode « communication » (annulation d'écho du système, volume d'appel),
 * puis casque Bluetooth / filaire / USB, haut-parleur ou écouteur (CallPolicy.route).
 */
class CallAudio(ctx: Context) {
    private val am = ctx.getSystemService(AudioManager::class.java)
    private var savedMode = AudioManager.MODE_NORMAL
    private var active = false
    private var scoOn = false
    var speaker = CallPolicy.defaultSpeaker()
        private set

    private val headsetTypes = buildSet {
        add(AudioDeviceInfo.TYPE_WIRED_HEADSET)
        add(AudioDeviceInfo.TYPE_WIRED_HEADPHONES)
        add(AudioDeviceInfo.TYPE_USB_HEADSET)
        add(AudioDeviceInfo.TYPE_BLUETOOTH_SCO)
        add(AudioDeviceInfo.TYPE_HEARING_AID)
        if (Build.VERSION.SDK_INT >= 31) add(AudioDeviceInfo.TYPE_BLE_HEADSET)
    }

    private fun candidates(): List<AudioDeviceInfo> =
        if (Build.VERSION.SDK_INT >= 31) am.availableCommunicationDevices
        else am.getDevices(AudioManager.GET_DEVICES_OUTPUTS).toList()

    fun headsetAvailable(): Boolean = candidates().any { it.type in headsetTypes }

    fun start() {
        if (active) return
        active = true
        savedMode = am.mode
        am.mode = AudioManager.MODE_IN_COMMUNICATION
        apply()
    }

    fun setSpeaker(on: Boolean) {
        speaker = on
        if (active) apply()
    }

    /** Appelé quand un casque est branché ou débranché. */
    fun refresh() { if (active) apply() }

    fun route(): CallPolicy.Route = CallPolicy.route(headsetAvailable(), speaker)

    private fun apply() {
        val route = route()
        if (Build.VERSION.SDK_INT >= 31) {
            val list = am.availableCommunicationDevices
            val device = when (route) {
                CallPolicy.Route.HEADSET -> list.firstOrNull { it.type in headsetTypes }
                CallPolicy.Route.SPEAKER -> list.firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER }
                CallPolicy.Route.EARPIECE -> list.firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE }
            }
            if (device != null) am.setCommunicationDevice(device) else am.clearCommunicationDevice()
        } else {
            @Suppress("DEPRECATION")
            run {
                am.isSpeakerphoneOn = route == CallPolicy.Route.SPEAKER
                val bluetooth = candidates().any { it.type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO }
                if (route == CallPolicy.Route.HEADSET && bluetooth && !scoOn && am.isBluetoothScoAvailableOffCall) {
                    am.startBluetoothSco()
                    am.isBluetoothScoOn = true
                    scoOn = true
                } else if ((route != CallPolicy.Route.HEADSET || !bluetooth) && scoOn) {
                    am.isBluetoothScoOn = false
                    am.stopBluetoothSco()
                    scoOn = false
                }
            }
        }
    }

    fun stop() {
        if (!active) return
        active = false
        if (Build.VERSION.SDK_INT >= 31) am.clearCommunicationDevice()
        else @Suppress("DEPRECATION") run {
            am.isSpeakerphoneOn = false
            if (scoOn) { am.isBluetoothScoOn = false; am.stopBluetoothSco(); scoOn = false }
        }
        am.mode = savedMode
    }
}
