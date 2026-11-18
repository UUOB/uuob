package app.uuob

import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import org.json.JSONObject
import java.util.Calendar
import java.util.TimeZone

/**
 * What is coming up in the user's apps (events, tasks that are due), kept on
 * this phone for the alerts (shown as notifications at their time) and the
 * "Up next" widget, because UUOB is the one process that keeps running and
 * learns about changes made on other devices.
 *
 * Each app keeps its list in UUOB (see `Node::upcoming`: `{ date, time?,
 * end_date?, end?, tz?, title, body?, alert? }`, times on the wall clock of
 * `tz`, or of this phone without one). The node hands the whole list over
 * whenever it changes (see UuobService). It is kept on disk and the next
 * alert is armed with Android's alarms, so it rings when UUOB's process is
 * gone or the phone is asleep: nothing here needs the node.
 */
object Alerts {
  private const val FIRE = "app.uuob.ALERT"
  /** An alert found more than this late (the phone was off) is skipped. */
  private const val STALE = 60 * 60 * 1000L

  class Item(
    val key: String, val app: String, val appName: String, val title: String, val body: String,
    /** One of the calendar colours ("blue", "red", …), for the widget. */
    val color: String,
    /** When it starts (midnight of its day for one without a time) and ends (null: it doesn't). */
    val start: Long, val end: Long?, val timed: Boolean, val rings: Long?,
  ) {
    /** "9:30–10:00 · Room 2", on this phone's clock. */
    fun text(hour12: Boolean) = listOfNotNull(
      if (!timed) null else if (end != null) "${clock(start, hour12)}–${clock(end, hour12)}" else clock(start, hour12),
      body.ifEmpty { null },
    ).joinToString(" · ")
  }

  private fun prefs(ctx: Context) = ctx.getSharedPreferences("alerts", Context.MODE_PRIVATE)

  /** The node's new list (JSON `{ hour12, items }`). */
  fun save(ctx: Context, list: String) {
    val p = prefs(ctx)
    // The very first list: what is already past was never meant for this phone.
    if (!p.contains("since")) p.edit().putLong("since", System.currentTimeMillis()).apply()
    p.edit().putString("list", list).apply()
    arm(ctx)
    UpNext.refresh(ctx)
  }

  /** Alerts still to ring: not rung yet, and not more than STALE late (one
   *  that arrives a little late from another device still rings). */
  private fun waiting(ctx: Context, now: Long): List<Item> {
    val p = prefs(ctx)
    val since = maxOf(p.getLong("since", Long.MAX_VALUE), now - STALE)
    val rung = p.getStringSet("rung", emptySet())!!
    return items(ctx).filter { it.rings != null && it.rings > since && "${it.key}@${it.rings}" !in rung }
  }

  /** Sets the alarm for the next alert (after a change, a restart, a new time zone). */
  fun arm(ctx: Context) {
    val next = waiting(ctx, System.currentTimeMillis()).mapNotNull { it.rings }.minOrNull()
    val am = ctx.getSystemService(AlarmManager::class.java)
    val ring = PendingIntent.getBroadcast(
      ctx, 0, Intent(ctx, AlertReceiver::class.java).setAction(FIRE),
      PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )
    when {
      next == null -> am.cancel(ring)
      // To the minute when Android allows it; otherwise within a few minutes.
      Build.VERSION.SDK_INT < 31 || am.canScheduleExactAlarms() -> am.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, next, ring)
      else -> am.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, next, ring)
    }
  }

  /** The alarm rang: show what is due, then arm the next one. */
  fun fire(ctx: Context) {
    val p = prefs(ctx)
    val now = System.currentTimeMillis()
    val due = waiting(ctx, now).filter { it.rings!! <= now }
    for (a in due) show(ctx, a)
    // Remember what rang (for as long as it could ring again).
    val rung = p.getStringSet("rung", emptySet())!!.filter { (it.substringAfterLast('@').toLongOrNull() ?: 0) > now - STALE }
    p.edit().putStringSet("rung", (rung + due.map { "${it.key}@${it.rings}" }).toSet()).apply()
    arm(ctx)
    UpNext.refresh(ctx)
  }

  /** Opens the app `id` (UUOB's apps are app.uuob.<id>), if it is on this phone. */
  fun openApp(ctx: Context, id: String): Intent? = ctx.packageManager.getLaunchIntentForPackage("app.uuob.$id")

  private fun show(ctx: Context, a: Item) {
    val nm = ctx.getSystemService(NotificationManager::class.java)
    // One channel per app, so the user can quiet one app in Android's settings.
    val channel = "app:${a.app}"
    nm.createNotificationChannel(NotificationChannel(channel, a.appName, NotificationManager.IMPORTANCE_HIGH))
    // Tapping opens the app it came from (nothing to open if it isn't on this phone).
    val tap = openApp(ctx, a.app)?.let { PendingIntent.getActivity(ctx, a.key.hashCode(), it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT) }
    val n = NotificationCompat.Builder(ctx, channel)
      .setSmallIcon(R.drawable.ic_notify)
      .setSubText(a.appName) // "UUOB · Calendar" above the title
      .setContentTitle(a.title)
      .apply { a.text(hour12(ctx)).takeIf { it.isNotEmpty() }?.let { setContentText(it) } }
      .setCategory(NotificationCompat.CATEGORY_REMINDER)
      .setPriority(NotificationCompat.PRIORITY_HIGH)
      .apply { if (tap != null) setContentIntent(tap).setAutoCancel(true) }
      .build()
    nm.notify(a.key.hashCode(), n)
  }

  private fun list(ctx: Context) = try { JSONObject(prefs(ctx).getString("list", null) ?: "{}") } catch (e: Exception) { JSONObject() }

  fun hour12(ctx: Context) = list(ctx).optBoolean("hour12", false)

  /** What is coming up, as the node handed it over. */
  fun items(ctx: Context): List<Item> {
    val list = list(ctx).optJSONArray("items") ?: return emptyList()
    return (0 until list.length()).mapNotNull { i ->
      val o = list.optJSONObject(i) ?: return@mapNotNull null
      val tz = o.optString("tz")
      val date = o.optString("date")
      val time = o.optString("time")
      val timed = time.isNotEmpty()
      val start = instant(date, time.ifEmpty { "00:00" }, tz) ?: return@mapNotNull null
      val endTime = o.optString("end")
      val endDate = o.optString("end_date")
      val end = when {
        endTime.isNotEmpty() && timed -> instant(endDate.ifEmpty { date }, endTime, tz)?.let { if (endDate.isEmpty() && it <= start) it + DAY else it }
        endDate.isNotEmpty() -> instant(endDate, "00:00", tz)?.plus(DAY) // the end of its last day
        else -> null
      }
      val app = o.optString("app")
      val rings = (o.opt("alert") as? Number)?.takeIf { timed }?.let { start - it.toLong() * 60_000 }
      Item("$app/${o.optString("id")}", app, o.optString("app_name", app), o.optString("title"), o.optString("body"), o.optString("color"), start, end, timed, rings)
    }.sortedBy { it.start }
  }

  private const val DAY = 24 * 60 * 60 * 1000L

  /** When the wall clock of `tz` (or this phone's) shows `date` `time`. */
  private fun instant(date: String, time: String, tz: String): Long? {
    val d = date.split("-").mapNotNull { it.toIntOrNull() }
    val t = time.split(":").mapNotNull { it.toIntOrNull() }
    if (d.size != 3 || t.size != 2) return null
    val zone = TimeZone.getTimeZone(tz).takeIf { tz.isNotEmpty() && it.id == tz } ?: TimeZone.getDefault()
    return Calendar.getInstance(zone).apply {
      clear()
      set(d[0], d[1] - 1, d[2], t[0], t[1])
    }.timeInMillis
  }

  /** "9:30" or "9:30 AM" on this phone's clock. */
  fun clock(at: Long, hour12: Boolean): String {
    val c = Calendar.getInstance().apply { timeInMillis = at }
    val h = c.get(Calendar.HOUR_OF_DAY)
    val m = "%02d".format(c.get(Calendar.MINUTE))
    return if (hour12) "${(h + 11) % 12 + 1}:$m ${if (h < 12) "AM" else "PM"}" else "$h:$m"
  }
}

/** The alarm, and the moments alarms must be set again: after the phone was
 *  turned on, UUOB was updated, or the clock or time zone changed. */
class AlertReceiver : BroadcastReceiver() {
  override fun onReceive(ctx: Context, intent: Intent) {
    if (intent.action == "app.uuob.ALERT") Alerts.fire(ctx)
    else {
      Alerts.arm(ctx)
      UpNext.refresh(ctx)
    }
  }
}
