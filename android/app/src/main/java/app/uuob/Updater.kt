package app.uuob

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.os.Build
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File

/**
 * "Update to 0.4.1": appears when a newer UUOB is published. The core
 * downloads it and checks its signature (crates/uuob-app/src/update.rs);
 * the system installer then puts it in place, which closes UUOB.
 */
class Updater(private val ctx: Context, private val scope: CoroutineScope) {
  var version by mutableStateOf<String?>(null)
  var busy by mutableStateOf(false)
  var error by mutableStateOf<String?>(null)
  private var checked = 0L

  /** When the screen opens and when the user comes back to it, at most hourly. */
  fun check() {
    if (busy || System.currentTimeMillis() - checked < 3_600_000) return
    checked = System.currentTimeMillis()
    scope.launch {
      try {
        version = Core.call("update_check") as? String
      } catch (_: Core.Failed) {}
    }
  }

  fun install() {
    busy = true
    error = null
    scope.launch {
      try {
        val saved = Core.call("update_install", params("dir" to ctx.cacheDir.absolutePath)) as? JSONObject
        saved?.text("apk")?.let { withContext(Dispatchers.IO) { hand(File(it)) } }
      } catch (e: Exception) {
        if (e.message != "cancelled") error = e.message ?: "The update failed"
      }
      busy = false
    }
  }

  /** Gives the APK to the system installer. Returns only if that fails: on success Android replaces the app. */
  private suspend fun hand(file: File) {
    val installer = ctx.packageManager.packageInstaller
    val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
    params.setAppPackageName(ctx.packageName)
    if (Build.VERSION.SDK_INT >= 31) params.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
    val id = installer.createSession(params)
    val done = CompletableDeferred<String>()
    pending = done
    installer.openSession(id).use { session ->
      session.openWrite("update", 0, file.length()).use { out ->
        file.inputStream().use { it.copyTo(out) }
        session.fsync(out)
      }
      val flags = PendingIntent.FLAG_UPDATE_CURRENT or (if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0)
      session.commit(PendingIntent.getBroadcast(ctx, id, Intent(ctx, UpdateReceiver::class.java), flags).intentSender)
    }
    throw Exception(done.await())
  }

  companion object {
    /** The install in progress. It only ever fails: on success Android replaces the app. */
    var pending: CompletableDeferred<String>? = null
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
      PackageInstaller.STATUS_FAILURE_ABORTED -> Updater.pending?.complete("cancelled")
      else -> Updater.pending?.complete(intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: "The update could not be installed")
    }
  }
}

/** The Update button, in each screen's top bar. */
@Composable
fun UpdateButton(updater: Updater) {
  val version = updater.version ?: return
  Column(horizontalAlignment = Alignment.End, modifier = Modifier.padding(end = 8.dp)) {
    FilledTonalButton(onClick = updater::install, enabled = !updater.busy) {
      Text(if (updater.busy) "Updating…" else if (updater.error != null) "Try again" else "Update to $version")
    }
    updater.error?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
  }
}
