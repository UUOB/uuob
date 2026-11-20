package app.uuob.sdk

import android.app.Activity
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class ProveArgs {
  lateinit var nonce: String
}

/**
 * The Android half of the SDK: ask UUOB on this phone, through the system,
 * for its answer to `nonce`. Only the real UUOB gets the broadcast (it names
 * the package app.uuob), so an app that took UUOB's port cannot answer it.
 * The SDK compares this answer with the one from the port (`GET /api?prove=`).
 */
@TauriPlugin
class SdkPlugin(private val activity: Activity) : Plugin(activity) {
  /** `{ proof }`: UUOB's answer, or null (UUOB is not installed, or its port is not its own). */
  @Command
  fun prove(invoke: Invoke) {
    val args = invoke.parseArgs(ProveArgs::class.java)
    val intent = Intent("app.uuob.PROVE")
      .setPackage("app.uuob")
      .putExtra("nonce", args.nonce)
      .addFlags(Intent.FLAG_INCLUDE_STOPPED_PACKAGES)
    val answer = object : BroadcastReceiver() {
      override fun onReceive(ctx: Context, i: Intent) {
        invoke.resolve(JSObject().put("proof", resultData))
      }
    }
    activity.sendOrderedBroadcast(intent, null, answer, null, Activity.RESULT_OK, null, null)
  }
}
