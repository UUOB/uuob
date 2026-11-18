package app.uuob

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import org.json.JSONArray
import org.json.JSONObject

/**
 * Hands each app on this phone what is its own, for the app to show:
 * - what is coming up in it, for its widget (Calendar's "Up next"): UUOB
 *   keeps running and learns of changes from other devices, the widget has
 *   to draw while the app is closed. Each app (app.uuob.<id>) gets only its
 *   own items, `{ hour12, items }`, on `app.uuob.UPCOMING`.
 * - an alert that is due, on `app.uuob.SHOW_ALERT`, which the app shows as
 *   its own notification. An app without that receiver (not on this phone,
 *   or too old) shows nothing: UUOB never shows apps' alerts itself.
 * Only apps signed like UUOB can receive either (`app.uuob.permission.UPCOMING`).
 */
object ToApps {
  private const val LIST = "app.uuob.UPCOMING"
  private const val ALERT = "app.uuob.SHOW_ALERT"
  private const val PERMISSION = "app.uuob.permission.UPCOMING"

  /** Sends every app its part of the list `Alerts` keeps. */
  fun send(ctx: Context) {
    val all = Alerts.list(ctx)
    val items = all.optJSONArray("items") ?: JSONArray()
    @Suppress("DEPRECATION")
    val receivers = ctx.packageManager.queryBroadcastReceivers(Intent(LIST), 0)
    for (r in receivers) {
      val pkg = r.activityInfo.packageName
      if (!pkg.startsWith("app.uuob.")) continue
      val id = pkg.removePrefix("app.uuob.")
      val mine = JSONArray()
      for (i in 0 until items.length()) items.optJSONObject(i)?.takeIf { it.optString("app") == id }?.let { mine.put(it) }
      val list = JSONObject().put("hour12", all.optBoolean("hour12", false)).put("items", mine).toString()
      val intent = Intent(LIST).setClassName(pkg, r.activityInfo.name).putExtra("list", list).addFlags(Intent.FLAG_INCLUDE_STOPPED_PACKAGES)
      ctx.sendBroadcast(intent, PERMISSION)
    }
  }

  /** Has the app `app` show an alert: `key` tells alerts apart, `text` is "9:30–10:00 · Room 2". */
  fun alert(ctx: Context, app: String, key: String, title: String, text: String) {
    val pkg = "app.uuob.$app"
    @Suppress("DEPRECATION")
    val r = ctx.packageManager.queryBroadcastReceivers(Intent(ALERT).setPackage(pkg), 0).firstOrNull() ?: return
    val intent = Intent(ALERT).setClassName(pkg, r.activityInfo.name)
      .putExtra("key", key).putExtra("title", title).putExtra("text", text)
      .addFlags(Intent.FLAG_INCLUDE_STOPPED_PACKAGES or Intent.FLAG_RECEIVER_FOREGROUND)
    ctx.sendBroadcast(intent, PERMISSION)
  }
}

/** An app asking for its list (just installed, a widget just added). */
class AskReceiver : BroadcastReceiver() {
  override fun onReceive(ctx: Context, intent: Intent) {
    if (intent.action == "app.uuob.SEND_UPCOMING") ToApps.send(ctx)
  }
}
