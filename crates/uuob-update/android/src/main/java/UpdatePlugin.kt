package app.uuob.update

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.os.Build
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlin.concurrent.thread

@InvokeArg
class DownloadArgs {
  lateinit var url: String
  lateinit var name: String
}

@InvokeArg
class InstallArgs {
  lateinit var path: String
}

/** The Android half of in-app updates: fetch a file, and hand an APK to the system installer. */
@TauriPlugin
class UpdatePlugin(private val activity: Activity) : Plugin(activity) {
  companion object {
    /** The install in progress. It only ever fails: on success Android replaces the app. */
    var pending: Invoke? = null
  }

  /** Saves `url` in the app's cache as `name` and returns its path. */
  @Command
  fun download(invoke: Invoke) {
    val args = invoke.parseArgs(DownloadArgs::class.java)
    thread {
      try {
        val file = File(activity.cacheDir, args.name)
        val http = URL(args.url).openConnection() as HttpURLConnection
        http.connectTimeout = 15000
        http.readTimeout = 30000
        if (http.responseCode != 200) throw IOException("HTTP ${http.responseCode}")
        http.inputStream.use { input -> file.outputStream().use { input.copyTo(it) } }
        invoke.resolve(JSObject().put("path", file.absolutePath))
      } catch (e: Exception) {
        invoke.reject(e.message ?: "The download failed")
      }
    }
  }

  /** Installs the APK at `path` over this app. Android checks it is signed by the same author. */
  @Command
  fun install(invoke: Invoke) {
    val file = File(invoke.parseArgs(InstallArgs::class.java).path)
    val installer = activity.packageManager.packageInstaller
    val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
    params.setAppPackageName(activity.packageName)
    if (Build.VERSION.SDK_INT >= 31) params.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
    val id = installer.createSession(params)
    installer.openSession(id).use { session ->
      session.openWrite("update", 0, file.length()).use { out ->
        file.inputStream().use { it.copyTo(out) }
        session.fsync(out)
      }
      pending = invoke
      val flags = PendingIntent.FLAG_UPDATE_CURRENT or (if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0)
      val result = PendingIntent.getBroadcast(activity, id, Intent(activity, UpdateReceiver::class.java), flags)
      session.commit(result.intentSender)
    }
  }
}

/** Hears from the system installer: show its question to the user, or report why it failed. */
class UpdateReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    when (intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
      PackageInstaller.STATUS_PENDING_USER_ACTION -> {
        @Suppress("DEPRECATION")
        val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT) ?: return
        context.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
      }
      PackageInstaller.STATUS_SUCCESS -> {}
      PackageInstaller.STATUS_FAILURE_ABORTED -> settle("cancelled")
      else -> settle(intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: "The update could not be installed")
    }
  }

  private fun settle(message: String) {
    UpdatePlugin.pending?.reject(message)
    UpdatePlugin.pending = null
  }
}
