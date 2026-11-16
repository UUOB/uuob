package app.uuob

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * Starts UUOB when the phone turns on (after it is first unlocked). Android
 * only calls it once the user switched on "Start when the phone turns on"
 * (Devices, This phone): until then it is disabled in the manifest.
 */
class BootReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (intent.action == Intent.ACTION_BOOT_COMPLETED) context.startForegroundService(Intent(context, UuobService::class.java))
  }
}

/** Starts UUOB again after an update, which closes it. */
class UpdatedReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (intent.action == Intent.ACTION_MY_PACKAGE_REPLACED) context.startForegroundService(Intent(context, UuobService::class.java))
  }
}
