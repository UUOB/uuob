package app.uuob

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import org.json.JSONArray
import org.json.JSONObject
import java.security.MessageDigest
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Hands each app on this phone what is its own, for the app to show:
 * - what is coming up in it, for its widget (a calendar's "Up next"): UUOB
 *   keeps running and learns of changes from other devices, the widget has
 *   to draw while the app is closed. Each app gets only its own items,
 *   `{ hour12, items }`, on `app.uuob.UPCOMING`.
 * - an alert that is due, on `app.uuob.SHOW_ALERT`, which the app shows as
 *   its own notification. An app without that receiver shows nothing: UUOB
 *   never shows apps' alerts itself.
 *
 * Who gets it is what the user allowed in UUOB, not who built the app: a
 * connected app says which Android package it is (`packages` in the list,
 * see `Node::set_android_package`). The first time UUOB delivers to that
 * package it remembers its signing certificate, and from then on delivers
 * only while the installed app still has it. Another app installed under the
 * same name gets nothing until the user allows it again (a new connection,
 * a new pin). Every broadcast carries `from`, a PendingIntent made by UUOB:
 * Android vouches that its creator is app.uuob, which keeps out every app the
 * user never allowed. An allowed app could pass on a `from` it was given, so
 * it proves "from UUOB or an allowed app", not "from UUOB".
 */
object ToApps {
  private const val LIST = "app.uuob.UPCOMING"
  private const val ALERT = "app.uuob.SHOW_ALERT"

  /** Sends every allowed app its part of the list `Alerts` keeps. */
  fun send(ctx: Context) {
    val all = Alerts.list(ctx)
    val items = all.optJSONArray("items") ?: JSONArray()
    for ((app, pkg) in allowed(ctx, all)) {
      val receiver = receiver(ctx, LIST, pkg) ?: continue
      val mine = JSONArray()
      for (i in 0 until items.length()) items.optJSONObject(i)?.takeIf { it.optString("app") == app }?.let { mine.put(it) }
      val list = JSONObject().put("hour12", all.optBoolean("hour12", false)).put("items", mine).toString()
      ctx.sendBroadcast(signed(ctx, Intent(LIST).setClassName(pkg, receiver).putExtra("list", list)))
    }
  }

  /** Has the app `app` show an alert: `key` tells alerts apart, `text` is "9:30–10:00 · Room 2". */
  fun alert(ctx: Context, app: String, key: String, title: String, text: String) {
    for ((id, pkg) in allowed(ctx, Alerts.list(ctx))) {
      if (id != app) continue
      val receiver = receiver(ctx, ALERT, pkg) ?: continue
      val intent = Intent(ALERT).setClassName(pkg, receiver).putExtra("key", key).putExtra("title", title).putExtra("text", text)
      ctx.sendBroadcast(signed(ctx, intent).addFlags(Intent.FLAG_RECEIVER_FOREGROUND))
    }
  }

  /** (app id, package) for each bound package whose certificate is the one pinned for its connection. */
  private fun allowed(ctx: Context, list: JSONObject): List<Pair<String, String>> {
    val packages = list.optJSONObject("packages") ?: JSONObject()
    val pins = ctx.getSharedPreferences("pins", Context.MODE_PRIVATE)
    val bound = mutableSetOf<String>()
    val out = mutableListOf<Pair<String, String>>()
    for (app in packages.keys()) {
      val byPackage = packages.optJSONObject(app) ?: continue
      for (pkg in byPackage.keys()) {
        val pin = "${byPackage.optString(pkg)}/$pkg" // connection/package
        bound += pin
        val cert = certificate(ctx, pkg) ?: continue // not installed
        val known = pins.getString(pin, null)
        if (known == null) pins.edit().putString(pin, cert).apply()
        if (known == null || known == cert) out += app to pkg
      }
    }
    // Connections that ended take their pins along.
    pins.all.keys.filter { it !in bound }.takeIf { it.isNotEmpty() }?.let { gone -> pins.edit().apply { gone.forEach(::remove) }.apply() }
    return out
  }

  /** SHA-256 of the package's signing certificates, or null when it isn't installed. */
  @Suppress("DEPRECATION")
  private fun certificate(ctx: Context, pkg: String): String? = try {
    val signatures = if (Build.VERSION.SDK_INT >= 28) {
      ctx.packageManager.getPackageInfo(pkg, PackageManager.GET_SIGNING_CERTIFICATES).signingInfo?.apkContentsSigners
    } else {
      ctx.packageManager.getPackageInfo(pkg, PackageManager.GET_SIGNATURES).signatures
    }
    val sha = MessageDigest.getInstance("SHA-256")
    signatures.orEmpty().map { sha.digest(it.toByteArray()).joinToString("") { b -> "%02x".format(b) } }.sorted().joinToString(",")
  } catch (e: PackageManager.NameNotFoundException) {
    null
  }

  /** The class in `pkg` that receives `action`, if it declares one. */
  @Suppress("DEPRECATION")
  private fun receiver(ctx: Context, action: String, pkg: String) =
    ctx.packageManager.queryBroadcastReceivers(Intent(action).setPackage(pkg), 0).firstOrNull()?.activityInfo?.name

  /** Adds `from` (proof the broadcast is UUOB's) and wakes a stopped app. */
  private fun signed(ctx: Context, intent: Intent): Intent {
    val from = PendingIntent.getBroadcast(ctx, 0, Intent("app.uuob.FROM").setPackage(ctx.packageName), PendingIntent.FLAG_IMMUTABLE)
    return intent.putExtra("from", from).addFlags(Intent.FLAG_INCLUDE_STOPPED_PACKAGES)
  }
}

/** An app asking for its list (just installed, a widget just added). Anyone
 *  may ask: UUOB then sends each allowed app its own list, nothing more. */
class AskReceiver : BroadcastReceiver() {
  override fun onReceive(ctx: Context, intent: Intent) {
    if (intent.action == "app.uuob.SEND_UPCOMING") ToApps.send(ctx)
  }
}

/**
 * An app on this phone checks that the server on UUOB's port is UUOB before
 * it sends its key there: another app could have taken the port while UUOB
 * was not running. The app sends an ordered broadcast to this package with a
 * random `nonce`; only the real UUOB receives it (the broadcast names
 * app.uuob). The answer is `uuob_api::proof(nonce)`, which the server on the
 * port gives too (`GET /api?prove=`); the app compares the two. While the port
 * is not UUOB's there is no answer.
 */
class ProveReceiver : BroadcastReceiver() {
  override fun onReceive(ctx: Context, intent: Intent) {
    val nonce = intent.getStringExtra("nonce") ?: return
    val done = goAsync()
    Core.scope.launch {
      try {
        runCatching { Core.start(ctx.applicationContext) }
        // The server takes the port just after the node starts.
        var proof: String? = null
        for (i in 0 until 40) {
          proof = app.uuob.core.prove(nonce)
          if (proof != null) break
          delay(50)
        }
        done.resultData = proof
      } finally {
        done.finish()
      }
    }
  }
}
