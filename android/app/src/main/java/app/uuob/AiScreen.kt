package app.uuob

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.size
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/**
 * AI access. AI clients run on computers, so a phone doesn't connect new
 * ones; it changes what the existing ones may see, for all devices.
 */
@Composable
fun AiScreen(shell: Shell, top: @Composable () -> Unit, only: String, onBack: () -> Unit) {
  var clients by remember { mutableStateOf(listOf<Pair<String, JSONObject>>()) }
  var containers by remember { mutableStateOf(listOf<JSONObject>()) }
  var removing by remember { mutableStateOf<Pair<String, JSONObject>?>(null) }

  suspend fun load() {
    clients = Core.obj("clients").entries().filter { (id, c) -> c.text("app") == null && id == only }
    containers = Core.list("containers")
  }
  var loaded by remember { mutableStateOf(false) }
  LaunchedEffect(shell.tick) { runCatching { load(); loaded = true } }
  LaunchedEffect(loaded, clients) { if (loaded && clients.isEmpty()) onBack() }

  // Access is given per app and covers all of that app's lists, calendars, …
  val apps = containers.distinctBy { it.optString("owner_app") }
  fun appName(k: JSONObject) = containers.firstOrNull { it.optString("id") == k.optString("owner_app") }?.optString("name") ?: k.optString("name")
  fun level(cid: String, k: JSONObject) = k.objects("access").firstOrNull { it.optString("client") == "ai:$cid" }?.optString("access") ?: "none"

  Page(clients.firstOrNull()?.second?.optString("name") ?: "", "", top, onBack) {
    clients.forEach { (cid, c) ->
      block(cid) {
        Section {
          Item(
            colors = clearItem(),
            leadingContent = {
              androidx.compose.foundation.layout.Box(
                Modifier.size(40.dp).clip(androidx.compose.foundation.shape.RoundedCornerShape(11.dp)).background(Tint.apps[4]),
                contentAlignment = androidx.compose.ui.Alignment.Center,
              ) { Ico("ai", null, tint = androidx.compose.ui.graphics.Color.White) }
            },
            headlineContent = { Text(c.optString("name"), style = MaterialTheme.typography.titleMedium) },
            supportingContent = { Text("Connected ${day(c.optLong("created_at"))}") },
            trailingContent = { IconBtn(onClick = { removing = cid to c }) { Ico("trash", "Disconnect") } },
          )
          apps.forEach { k ->
            Line()
            Column(Modifier.padding(16.dp)) {
              Text(appName(k), style = MaterialTheme.typography.titleSmall)
              Choice(listOf("none" to "No access", "read" to "Read", "write" to "Read & write"), level(cid, k), Modifier.fillMaxWidth().padding(top = 8.dp)) { v ->
                shell.act {
                  Core.call("set_grant", params("client" to "ai:$cid", "app" to k.optString("owner_app"), "access" to v.takeIf { it != "none" }))
                  load()
                }
              }
            }
          }
        }
      }
    }
  }

  removing?.let { (cid, c) ->
    Confirm("Disconnect “${c.optString("name")}”?", "Its key stops working immediately on all your devices.", "Disconnect",
      onConfirm = { removing = null; shell.act { Core.call("delete_client", params("id" to cid)); load() } },
      onDismiss = { removing = null })
  }
}
