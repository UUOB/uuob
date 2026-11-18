package app.uuob

import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager

/**
 * Starts UUOB when the phone turns on (after it is first unlocked). Android
 * only calls it once the user switched on "Start when the phone turns on"
 * (Devices, This phone): until then it is disabled in the manifest.
 */
class BootReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (intent.action == Intent.ACTION_BOOT_COMPLETED) context.startForegroundService(Intent(context, UuobService::class.java))
  }

  companion object {
    private fun component(ctx: Context) = ComponentName(ctx, BootReceiver::class.java)

    /** Is "Start when the phone turns on" on? Android keeps that, by this class's name. */
    fun isOn(ctx: Context) = ctx.packageManager.getComponentEnabledSetting(component(ctx)) == PackageManager.COMPONENT_ENABLED_STATE_ENABLED

    fun set(ctx: Context, on: Boolean) {
      val state = if (on) PackageManager.COMPONENT_ENABLED_STATE_ENABLED else PackageManager.COMPONENT_ENABLED_STATE_DEFAULT
      ctx.packageManager.setComponentEnabledSetting(component(ctx), state, PackageManager.DONT_KILL_APP)
    }
  }
}

/** Starts UUOB again after an update, which closes it. */
class UpdatedReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (intent.action == Intent.ACTION_MY_PACKAGE_REPLACED) context.startForegroundService(Intent(context, UuobService::class.java))
  }
}
