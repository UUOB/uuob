package app.uuob

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import androidx.core.app.NotificationCompat

/**
 * Keeps UUOB's process alive while you use other apps, so Tasks, Calendar
 * and your other devices can reach it. Shows a quiet "UUOB is running"
 * notification, as Android requires. It also starts UUOB itself (the node
 * and its local server) when the screen hasn't: after the phone was turned
 * on, see BootReceiver.
 */
class UuobService : Service() {
  companion object {
    init { System.loadLibrary("uuob_app_lib") }
    @Volatile private var watching = false
  }

  private external fun startNode(dataDir: String)
  /** What is coming up in the user's apps, once it differs from `known` (waits till then). */
  private external fun nextUpcoming(known: String): String

  override fun onBind(intent: Intent?) = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    val channel = "background"
    val nm = getSystemService(NotificationManager::class.java)
    nm.createNotificationChannel(NotificationChannel(channel, "Running in the background", NotificationManager.IMPORTANCE_LOW))
    val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
    val n = NotificationCompat.Builder(this, channel)
      .setSmallIcon(R.drawable.ic_notify)
      .setContentTitle("UUOB is running")
      .setContentText("Your apps and devices stay in sync")
      .setContentIntent(open)
      .setOngoing(true)
      .build()
    if (Build.VERSION.SDK_INT >= 34) startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
    else startForeground(1, n)
    // Apps show their own alerts now: drop the per-app channels UUOB once had.
    nm.notificationChannels.filter { it.id.startsWith("app:") }.forEach { nm.deleteNotificationChannel(it.id) }
    startNode(dataDir.absolutePath)
    // Hand what is coming up to Alerts (which keeps time for the alerts, also once this process is gone, and hands each app its part: its widget's list, its alerts when due).
    if (!watching) {
      watching = true
      val ctx = applicationContext
      Thread({
        var known = ""
        while (true) {
          known = nextUpcoming(known)
          Alerts.save(ctx, known)
        }
      }, "upcoming").apply { isDaemon = true }.start()
    }
    // The node lives in the app process; if Android kills it, the next launch starts both again.
    return START_NOT_STICKY
  }
}
