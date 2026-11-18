package app.uuob

import android.app.Activity
import android.app.AlarmManager
import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.net.Uri
import android.os.Bundle
import android.text.TextPaint
import android.text.TextUtils
import android.util.TypedValue
import android.widget.RemoteViews
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Locale

/**
 * The "Up next" widget: the next hours of the day as a timeline, with what is
 * coming up in all of the user's apps (see Alerts for where the list comes
 * from): events as blocks as long as they last, a red line at the time now,
 * and things for the whole day (or still due from before) as chips on top.
 * It is drawn as one picture; over it, one tap area per hour opens the app
 * of what is in that hour. Drawn again when the list changes, an alert rings,
 * the clock changes, and every quarter of an hour while the phone is awake.
 */
class UpNext : AppWidgetProvider() {
  companion object {
    private const val REDRAW = "app.uuob.REDRAW"
    private const val QUARTER = 15 * 60 * 1000L

    fun refresh(ctx: Context) {
      val m = AppWidgetManager.getInstance(ctx)
      val ids = m.getAppWidgetIds(ComponentName(ctx, UpNext::class.java))
      for (id in ids) draw(ctx, m, id)
      tick(ctx, ids.isNotEmpty())
    }

    /** Redraws every quarter of an hour (the line for now moves), only while the phone is awake. */
    private fun tick(ctx: Context, on: Boolean) {
      val again = PendingIntent.getBroadcast(ctx, 1, Intent(ctx, UpNext::class.java).setAction(REDRAW), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
      val am = ctx.getSystemService(AlarmManager::class.java)
      if (on) am.setInexactRepeating(AlarmManager.RTC, System.currentTimeMillis() + QUARTER, QUARTER, again) else am.cancel(again)
    }

    private fun draw(ctx: Context, m: AppWidgetManager, id: Int) {
      val o = m.getAppWidgetOptions(id)
      val w = o.getInt(AppWidgetManager.OPTION_APPWIDGET_MIN_WIDTH).takeIf { it > 0 } ?: 320
      val h = o.getInt(AppWidgetManager.OPTION_APPWIDGET_MAX_HEIGHT).takeIf { it > 0 } ?: 180
      // Drawn light and dark: the phone shows the one for its mode, also after it changes.
      val t = Timeline(ctx, w, h, night = false)
      val views = RemoteViews(ctx.packageName, R.layout.widget_up_next)
      views.setImageViewBitmap(R.id.timeline_day, t.bitmap)
      views.setImageViewBitmap(R.id.timeline_night, Timeline(ctx, w, h, night = true).bitmap)
      views.removeAllViews(R.id.slots)
      t.slots.forEachIndexed { i, app ->
        val open = Intent(ctx, OpenApp::class.java).putExtra("app", app).setData(Uri.parse("uuob-widget:$id/$i"))
        val slot = RemoteViews(ctx.packageName, R.layout.widget_slot)
        slot.setOnClickPendingIntent(R.id.slot, PendingIntent.getActivity(ctx, id * 100 + i, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT))
        views.addView(R.id.slots, slot)
      }
      m.updateAppWidget(id, views)
    }
  }

  override fun onUpdate(ctx: Context, m: AppWidgetManager, ids: IntArray) {
    for (id in ids) draw(ctx, m, id)
    tick(ctx, true)
  }

  override fun onAppWidgetOptionsChanged(ctx: Context, m: AppWidgetManager, id: Int, options: Bundle) = draw(ctx, m, id)

  override fun onDisabled(ctx: Context) = tick(ctx, false)

  override fun onReceive(ctx: Context, intent: Intent) {
    if (intent.action == REDRAW) refresh(ctx) else super.onReceive(ctx, intent)
  }
}

/**
 * The picture, `wDp` × `hDp`: a band on top (the day, and chips for the whole
 * day) and then the hours from just before now, all of the same height, so
 * `slots` (the app to open per band, top first) can lie over it as equal rows.
 */
private class Timeline(val ctx: Context, wDp: Int, hDp: Int, val night: Boolean) {
  val d = ctx.resources.displayMetrics.density
  // UUOB's colours (ui/tokens.css; the background is res/values*/widget_colors.xml).
  val text = (if (night) 0xFFF5F5F7 else 0xFF1D1D1F).toInt()
  val text2 = alpha(text, 0.62f)
  val accent = (if (night) 0xFF0A84FF else 0xFF007AFF).toInt()
  val red = (if (night) 0xFFFF453A else 0xFFFF3B30).toInt()
  val bitmap: Bitmap = Bitmap.createBitmap((wDp * d).toInt(), (hDp * d).toInt(), Bitmap.Config.ARGB_8888)
  val slots = mutableListOf<String>()

  fun dp(v: Float) = v * d
  fun sp(v: Float) = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, v, ctx.resources.displayMetrics)
  fun paint(size: Float, color: Int, bold: Boolean = false) = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
    textSize = sp(size)
    this.color = color
    typeface = Typeface.create(Typeface.DEFAULT, if (bold) Typeface.BOLD else Typeface.NORMAL)
  }
  fun alpha(c: Int, a: Float) = Color.argb((a * 255).toInt(), Color.red(c), Color.green(c), Color.blue(c))
  /** `c` lightly over the widget's background, solid (the hour lines don't show through). */
  val bg = (if (night) 0xFF1C1C1F else 0xFFFFFFFF).toInt()
  fun tinted(c: Int): Int {
    val mix = { a: Int, b: Int -> (a * tint + b * (1 - tint)).toInt() }
    return Color.rgb(mix(Color.red(c), Color.red(bg)), mix(Color.green(c), Color.green(bg)), mix(Color.blue(c), Color.blue(bg)))
  }
  fun fill(color: Int) = Paint(Paint.ANTI_ALIAS_FLAG).apply { this.color = color }

  /** The calendar colours (ui/tokens.css), light and dark. */
  fun color(name: String): Int = when (name) {
    "red" -> if (night) 0xFFFF453A else 0xFFFF3B30
    "orange" -> if (night) 0xFFFF9F0A else 0xFFFF9500
    "yellow" -> if (night) 0xFFFFD60A else 0xFFFFCC00
    "green" -> if (night) 0xFF30D158 else 0xFF34C759
    "teal" -> if (night) 0xFF40C8E0 else 0xFF30B0C7
    "purple" -> if (night) 0xFFBF5AF2 else 0xFFAF52DE
    "pink" -> if (night) 0xFFFF375F else 0xFFFF2D55
    else -> if (night) 0xFF0A84FF else 0xFF007AFF
  }.toInt()

  val tint = if (night) 0.32f else 0.2f

  init {
    val c = Canvas(bitmap)
    val hour12 = Alerts.hour12(ctx)
    val now = System.currentTimeMillis()
    val tomorrow = Calendar.getInstance().apply {
      set(Calendar.HOUR_OF_DAY, 0); set(Calendar.MINUTE, 0); set(Calendar.SECOND, 0); set(Calendar.MILLISECOND, 0)
      add(Calendar.DATE, 1)
    }.timeInMillis
    val items = Alerts.items(ctx).filter { it.end == null || it.end > now }

    // Bands: the top one, then an hour each (about 46dp).
    val bands = (hDp / 46).coerceIn(4, 15)
    val band = bitmap.height.toFloat() / bands
    val hours = bands - 1
    val from = Calendar.getInstance().apply {
      timeInMillis = now
      set(Calendar.SECOND, 0); set(Calendar.MILLISECOND, 0)
      if (hours >= 4) add(Calendar.MINUTE, -30) // a little of what just began
      set(Calendar.MINUTE, 0)
    }.timeInMillis
    val to = from + hours * 3_600_000L
    val y = { t: Long -> band + (t - from) / 3_600_000f * band }
    val pad = dp(14f)
    val left = pad + dp(if (hour12) 60f else 44f)
    val right = bitmap.width - pad

    // The top band: the day, then chips for what is for the whole day (or still due).
    val dayPaint = paint(13f, accent, bold = true)
    val day = SimpleDateFormat("EEEE d", Locale.getDefault()).format(now).uppercase(Locale.getDefault())
    c.drawText(day, pad, band / 2 + dayPaint.textSize / 3, dayPaint)
    val chips = items.filter { !it.timed && it.start < tomorrow }
    val chipPaint = paint(13f, text)
    val chipH = minOf(dp(26f), band - dp(8f))
    var x = pad + dayPaint.measureText(day) + dp(10f)
    for ((i, it) in chips.withIndex()) {
      val more = chips.size - i - 1
      val moreText = if (more > 0) "+$more" else ""
      val room = right - x - (if (more > 0) chipPaint.measureText(moreText) + dp(8f) else 0f)
      if (room < dp(56f)) {
        c.drawText("+${more + 1}", x, band / 2 + chipPaint.textSize / 3, paint(13f, text2))
        break
      }
      val label = TextUtils.ellipsize(it.title, chipPaint, room - dp(16f), TextUtils.TruncateAt.END).toString()
      val r = RectF(x, (band - chipH) / 2, x + chipPaint.measureText(label) + dp(16f), (band + chipH) / 2)
      c.drawRoundRect(r, dp(7f), dp(7f), fill(tinted(color(it.color))))
      c.drawText(label, x + dp(8f), r.centerY() + chipPaint.textSize / 3, chipPaint)
      x = r.right + dp(6f)
    }
    slots += chips.firstOrNull()?.app ?: "calendar"

    // The hours: a line and a label each.
    val line = Paint().apply { color = alpha(text, 0.12f); strokeWidth = dp(1f) }
    val hourPaint = paint(12f, text2)
    val cal = Calendar.getInstance()
    for (i in 0 until hours) {
      val t = from + i * 3_600_000L
      cal.timeInMillis = t
      val h = cal.get(Calendar.HOUR_OF_DAY)
      val label = when {
        h == 0 -> SimpleDateFormat("EEE", Locale.getDefault()).format(t) // a new day
        hour12 && h == 12 -> "noon"
        hour12 -> "${(h + 11) % 12 + 1} ${if (h < 12) "AM" else "PM"}"
        else -> "$h:00"
      }
      val ly = y(t)
      if (kotlin.math.abs(ly - y(now)) > dp(10f)) c.drawText(label, pad, ly + hourPaint.textSize / 3, hourPaint)
      c.drawLine(left, ly, right, ly, line)
    }

    // Events (and tasks at a time, as half an hour), side by side where they overlap.
    class Block(val item: Alerts.Item, val a: Long, val b: Long) { var column = 0; var columns = 1 }
    val blocks = items.filter { it.timed }
      .map { Block(it, maxOf(it.start, from), minOf(it.end ?: (it.start + 30 * 60_000L), to)) }
      .filter { it.b > it.a }
      .sortedBy { it.a }
    var i = 0
    while (i < blocks.size) {
      // A run of blocks that overlap one another shares the width.
      var j = i
      var end = blocks[i].b
      while (j + 1 < blocks.size && blocks[j + 1].a < end) end = maxOf(end, blocks[++j].b)
      val ends = mutableListOf<Long>()
      for (k in i..j) {
        val free = ends.indexOfFirst { it <= blocks[k].a }
        blocks[k].column = if (free >= 0) free else ends.size
        if (free >= 0) ends[free] = blocks[k].b else ends += blocks[k].b
      }
      for (k in i..j) blocks[k].columns = ends.size
      i = j + 1
    }
    val timePaint = paint(12f, text2)
    val titlePaint = paint(14f, text, bold = true)
    for (bl in blocks) {
      val col = color(bl.item.color)
      val w = (right - left - dp(4f)) / bl.columns
      val r = RectF(left + dp(4f) + bl.column * w, y(bl.a) + dp(1f), left + dp(4f) + (bl.column + 1) * w - dp(2f), y(bl.b) - dp(1f))
      c.drawRoundRect(r, dp(5f), dp(5f), fill(tinted(col)))
      c.drawRect(r.left, r.top, r.left + dp(3f), r.bottom, fill(col))
      c.save()
      c.clipRect(r)
      val tx = r.left + dp(8f)
      val room = r.right - tx - dp(4f)
      val time = Alerts.clock(bl.item.start, hour12)
      if (r.height() >= titlePaint.textSize + timePaint.textSize + dp(10f)) {
        c.drawText(TextUtils.ellipsize(time, timePaint, room, TextUtils.TruncateAt.END).toString(), tx, r.top + dp(4f) + timePaint.textSize, timePaint)
        c.drawText(TextUtils.ellipsize(bl.item.title, titlePaint, room, TextUtils.TruncateAt.END).toString(), tx, r.top + dp(7f) + timePaint.textSize + titlePaint.textSize, titlePaint)
      } else {
        // Too short for two lines: "9:30 Title" on one.
        val tw = timePaint.measureText("$time ")
        c.drawText("$time ", tx, r.centerY() + titlePaint.textSize / 3, timePaint)
        c.drawText(TextUtils.ellipsize(bl.item.title, titlePaint, room - tw, TextUtils.TruncateAt.END).toString(), tx + tw, r.centerY() + titlePaint.textSize / 3, titlePaint)
      }
      c.restore()
    }
    for (h in 0 until hours) {
      val a = from + h * 3_600_000L
      slots += blocks.firstOrNull { it.a < a + 3_600_000L && it.b > a }?.item?.app ?: "calendar"
    }

    // Now: a red line with a dot, and the time.
    val ny = y(now)
    val nowPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = red; strokeWidth = dp(1.5f) }
    c.drawLine(left, ny, right, ny, nowPaint)
    c.drawCircle(left, ny, dp(4f), nowPaint)
    val clockPaint = paint(12f, red, bold = true)
    c.drawText(Alerts.clock(now, hour12).replace(" ", " "), pad, ny + clockPaint.textSize / 3, clockPaint)
  }
}

/** A tap on the widget: opens an app (if it is on this phone) and is gone. */
class OpenApp : Activity() {
  override fun onCreate(state: Bundle?) {
    super.onCreate(state)
    intent.getStringExtra("app")?.let { Alerts.openApp(this, it) }?.let { startActivity(it) }
    finish()
  }
}
