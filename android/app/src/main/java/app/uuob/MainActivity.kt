package app.uuob

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.launch

/**
 * UUOB's screen. UUOB itself (the node) lives in the process, kept alive by
 * UuobService, so closing or swiping away this screen doesn't stop it: back
 * behaves like in any app.
 */
class MainActivity : ComponentActivity() {
  private var shell: Shell? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Android 13+ hides the "UUOB is running" notification unless allowed.
    if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
      requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
    }
    // Keep running in the background so apps and other devices can reach UUOB; this also starts it.
    startForegroundService(Intent(this, UuobService::class.java))
    startQuietly(intent)

    val updater = Updater(applicationContext, lifecycleScope)
    lifecycleScope.launch {
      try {
        Core.start(applicationContext)
      } catch (e: Core.Failed) {
        android.util.Log.e("UUOB", "UUOB could not start: ${e.message}")
      }
      // Coming back to UUOB: questions may be waiting (an app opened uuob://connect), and maybe an update.
      repeatOnLifecycle(Lifecycle.State.RESUMED) {
        shell?.loadAsks()
        updater.check()
      }
    }
    setContent {
      UuobTheme {
        val scope = rememberCoroutineScope()
        val shell = remember { Shell(scope).also { this.shell = it } }
        UuobApp(shell, updater)
      }
    }
  }

  // Notifications were allowed just now: have the service show "UUOB is running",
  // which Android hid when it was first posted.
  override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<String>, grantResults: IntArray) {
    super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    if (requestCode == 1 && grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED) {
      startForegroundService(Intent(this, UuobService::class.java))
    }
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    startQuietly(intent)
  }

  // An app found UUOB not running and opened uuob://start: start, then step
  // aside so the person lands back in that app.
  private fun startQuietly(intent: Intent?) {
    if (intent?.data?.host == "start") moveTaskToBack(true)
  }
}
