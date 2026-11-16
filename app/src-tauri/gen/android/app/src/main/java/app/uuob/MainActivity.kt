package app.uuob

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.content.res.Configuration
import android.graphics.Color
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  // No white flash while the page loads: let the themed window colour show through.
  override fun onWebViewCreate(webView: WebView) {
    webView.setBackgroundColor(Color.TRANSPARENT)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // UUOB keeps running after its screen is closed; when the screen comes
    // back, ask the running app to fill it again.
    screenCreated()
    // Keep running in the background so apps and other devices can reach UUOB.
    // Android 13+ hides the "UUOB is running" notification unless allowed.
    if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
      requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
    }
    startForegroundService(Intent(this, UuobService::class.java))
    // Back sends UUOB to the background instead of closing it: closing the
    // window would end the process, and with it the node that apps use.
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() { moveTaskToBack(true) }
    })
    startQuietly(intent)

    // Keep the web UI clear of the status bar, gesture bar and cutouts, and
    // paint the space behind them in the app's background colour.
    val night = (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
    val root = findViewById<android.view.View>(android.R.id.content)
    root.setBackgroundColor(Color.parseColor(if (night) "#0b0b0d" else "#f5f5f7"))
    WindowCompat.getInsetsController(window, root).apply {
      isAppearanceLightStatusBars = !night
      isAppearanceLightNavigationBars = !night
    }
    ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime())
      v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
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
  private external fun screenCreated()

  private fun startQuietly(intent: Intent?) {
    if (intent?.data?.host == "start") moveTaskToBack(true)
  }
}
