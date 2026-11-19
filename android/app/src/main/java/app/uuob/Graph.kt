package app.uuob

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Dns
import androidx.compose.material.icons.rounded.HelpOutline
import androidx.compose.material.icons.rounded.Laptop
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.json.JSONArray
import org.json.JSONObject

/**
 * The Network tab's graph. Where everything goes is worked out by UUOB
 * (`graph`, crates/uuob-app/src/screens.rs), the same on every platform;
 * this draws it: lines behind, a button per device, app, AI and person.
 * Tapping a bubble opens its group, the × folds it again; tapping anything
 * else shows what it is underneath.
 */
@Composable
fun Graph(shell: Shell, devices: List<JSONObject>) {
  var open by remember { mutableStateOf(setOf<String>()) }
  var picked by remember { mutableStateOf<JSONObject?>(null) }
  BoxWithConstraints(Modifier.fillMaxWidth()) {
    val width = maxWidth.value
    var graph by remember { mutableStateOf<JSONObject?>(null) }
    LaunchedEffect(shell.tick, width, open, devices) {
      runCatching { graph = Core.obj("graph", params("width" to width.toDouble(), "open" to JSONArray(open.toList()))) }
    }
    val g = graph ?: return@BoxWithConstraints
    val nodes = g.objects("nodes")
    // The picked node again after a refresh, else this device.
    val shown = nodes.firstOrNull { it.optString("id") == picked?.optString("id") } ?: nodes.firstOrNull { it.optBoolean("self") }
    Column {
      Card(
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
        shape = RoundedCornerShape(24.dp),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f)),
      ) {
        Box(Modifier.fillMaxWidth().height(g.optDouble("height").dp)) {
          val strong = MaterialTheme.colorScheme.outline
          val soft = MaterialTheme.colorScheme.outlineVariant
          Canvas(Modifier.fillMaxWidth().height(g.optDouble("height").dp)) {
            for (l in g.objects("lines")) {
              val style = l.optString("style")
              drawLine(
                if (style == "device") strong else soft,
                Offset(l.optDouble("x1").dp.toPx(), l.optDouble("y1").dp.toPx()),
                Offset(l.optDouble("x2").dp.toPx(), l.optDouble("y2").dp.toPx()),
                strokeWidth = (if (style == "device") 2.5 else 1.5).dp.toPx(),
                pathEffect = if (style == "person" || style == "link") PathEffect.dashPathEffect(floatArrayOf(5.dp.toPx(), 5.dp.toPx())) else null,
              )
            }
          }
          for (n in nodes) {
            GraphNode(n, selected = n.optString("id") == shown?.optString("id")) {
              when (n.optString("kind")) {
                "bubble" -> open = open + n.optString("group")
                "fold" -> open = open - n.optString("group")
                else -> picked = n
              }
            }
          }
        }
      }
      shown?.optJSONObject("about")?.let { about ->
        Section {
          Column(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 14.dp)) {
            if (about.optString("kind").isNotEmpty()) {
              Text(about.optString("kind").uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
            }
            Text(about.optString("title"), style = MaterialTheme.typography.titleLarge, modifier = Modifier.padding(top = 2.dp))
            if (about.optString("detail").isNotEmpty()) Caption(about.optString("detail"), Modifier.padding(top = 4.dp))
          }
        }
      }
    }
  }
}

/** One node: its shape centred on (x, y), its name underneath. */
@Composable
private fun GraphNode(n: JSONObject, selected: Boolean, onClick: () -> Unit) {
  val size = n.optDouble("size").dp
  val x = n.optDouble("x").dp
  val y = n.optDouble("y").dp
  val kind = n.optString("kind")
  val ring = if (selected && kind != "bubble" && kind != "fold") BorderStroke(3.dp, MaterialTheme.colorScheme.primary) else null
  val what = when (kind) {
    "bubble" -> "Open ${n.optInt("count")} ${n.optString("label")}"
    "fold" -> "Fold"
    else -> n.optString("label")
  }
  Surface(
    onClick = onClick,
    shape = when (kind) {
      "device" -> RoundedCornerShape(size * 0.3f)
      "app", "ai" -> RoundedCornerShape(size * 0.28f)
      else -> CircleShape
    },
    color = when (kind) {
      "app" -> Tint.apps[n.optInt("tint")]
      "ai" -> Tint.apps[4]
      "person" -> Tint.apps[6]
      "bubble" -> MaterialTheme.colorScheme.surfaceContainerHigh
      else -> MaterialTheme.colorScheme.surfaceContainerHighest
    },
    contentColor = when (kind) {
      "app", "ai", "person" -> Color.White
      else -> MaterialTheme.colorScheme.onSurface
    },
    border = ring ?: when {
      kind == "bubble" -> BorderStroke(1.5.dp, MaterialTheme.colorScheme.outline)
      n.optBoolean("dotted") -> BorderStroke(1.5.dp, MaterialTheme.colorScheme.outline)
      else -> null
    },
    modifier = Modifier.offset(x - size / 2, y - size / 2).size(size).semantics { contentDescription = what },
  ) {
    Box(contentAlignment = Alignment.Center) {
      when (kind) {
        "device" -> Icon(
          when {
            n.optString("id") == "unseen" -> Icons.Rounded.HelpOutline
            n.optBoolean("keeper") -> Icons.Rounded.Dns
            n.optString("platform") in setOf("android", "ios") -> Icons.Rounded.PhoneAndroid
            else -> Icons.Rounded.Laptop
          },
          null, Modifier.size(size * 0.45f),
        )
        "ai" -> Icon(Icons.Rounded.AutoAwesome, null, Modifier.size(size * 0.5f))
        "fold" -> Icon(Icons.Rounded.Close, null, Modifier.size(size * 0.5f))
        "bubble" -> Text(n.optString("count"), fontWeight = FontWeight.Bold, fontSize = (size.value * 0.3f).sp)
        else -> Text(n.optString("letter"), fontWeight = FontWeight.SemiBold, fontSize = (size.value * 0.42f).sp)
      }
    }
  }
  if (kind != "fold") {
    val self = n.optBoolean("self")
    Text(
      n.optString("label"),
      Modifier.offset(x - 50.dp, y + size / 2 + 4.dp).requiredWidth(100.dp),
      style = MaterialTheme.typography.labelSmall,
      fontWeight = if (self) FontWeight.SemiBold else FontWeight.Normal,
      color = if (self) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurfaceVariant,
      textAlign = TextAlign.Center, maxLines = 1, overflow = TextOverflow.Ellipsis,
    )
  }
}
