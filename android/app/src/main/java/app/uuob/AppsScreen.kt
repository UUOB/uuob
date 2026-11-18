package app.uuob

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.Group
import androidx.compose.material.icons.rounded.Widgets
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import org.json.JSONObject

/** One app as Apps shows it: its first container plus one per list, calendar, … */
private class AppGroup(val id: String, val name: String, val installed: Boolean, val cs: List<JSONObject>, val copies: List<Pair<String, JSONObject>>) {
  val records = cs.sumOf { it.optInt("records") }
  val bytes = cs.sumOf { it.optLong("size_bytes") + it.optLong("file_bytes") }
  val files = cs.sumOf { it.optInt("files") }
  val filesHere = cs.sumOf { it.optInt("files_here") }
  /** Connected storage devices that have all of it (of what is on this device). */
  val safe = cs.filter { it.optBoolean("here") && it.optInt("records") + it.optInt("files") > 0 }.minOfOrNull { it.optInt("copies") }
  val access = cs.first().objects("access")
  val parts = cs.filter { it.optString("id") != id }
}

private sealed interface AppSheet {
  data object Code : AppSheet
  data class Disconnect(val app: AppGroup) : AppSheet
  data class Wipe(val app: AppGroup) : AppSheet
}

@Composable
fun AppsScreen(shell: Shell, top: @Composable () -> Unit) {
  val ctx = LocalContext.current
  var groups by remember { mutableStateOf(listOf<AppGroup>()) }
  var keepers by remember { mutableIntStateOf(0) }
  var needed by remember { mutableIntStateOf(1) }
  var sheet by remember { mutableStateOf<AppSheet?>(null) }

  suspend fun load() {
    val containers = Core.list("containers")
    val apps = Core.obj("apps")
    val clients = Core.obj("clients").entries()
    keepers = Core.list("devices").count { it.optBoolean("keeper") }
    needed = Core.obj("status").optJSONObject("network")?.optInt("copies", 1) ?: 1
    groups = containers.map { it.optString("owner_app") }.distinct().map { id ->
      AppGroup(
        id, apps.optJSONObject(id)?.optString("name") ?: id, apps.optJSONObject(id)?.optBoolean("installed") == true,
        containers.filter { it.optString("owner_app") == id },
        // Copies of the app on devices without UUOB, each linked with a code.
        clients.filter { (_, c) -> c.text("app") == id && c.text("endpoint") != null }.sortedBy { (_, c) -> c.optLong("created_at") },
      )
    }
  }
  fun reload() = shell.act { load() }
  LaunchedEffect(shell.tick) { runCatching { load() } }

  fun storageNote(g: AppGroup) = listOfNotNull(
    if (g.files == 0) null else if (g.filesHere == g.files) plural(g.files, "file", "files") else "${g.filesHere} of ${g.files} files on this device",
    if (keepers == 0 || g.safe == null) null else if (g.safe >= needed) "On ${plural(g.safe, "storage device", "storage devices")}" else "Waiting for a storage device",
  ).joinToString(" · ")

  fun sharedNote(c: JSONObject): String? {
    val from = c.optJSONObject("from")
    val with = c.objects("shared_with")
    return when {
      from != null -> "From ${from.optString("name")}${if (c.optBoolean("can_write")) "" else " · view only"}"
      with.isNotEmpty() -> "Shared with ${with.joinToString { it.optString("name") }}"
      else -> null
    }
  }

  Page("Apps", "Apps like Tasks and Calendar keep their data here, each in its own place, synced to all your devices. An app sees only its own data unless you allow more.", top) {
    block {
      Caption("To share a list or a calendar with someone, open it in its app and choose Share.", Modifier.padding(horizontal = 4.dp, vertical = 8.dp))
      FilledTonalButton(onClick = { sheet = AppSheet.Code }) {
        Icon(Icons.Rounded.Add, null)
        Text("  Connect with a code")
      }
    }
    if (groups.isEmpty()) block {
      Empty(Icons.Rounded.Widgets, "No apps yet", "Install an app like Tasks or Calendar on this device and open it. It will ask to connect. On a device without UUOB, the app connects with a code.")
    }
    groups.forEach { g ->
      block(g.id) {
        Card(Modifier.fillMaxWidth().padding(top = 20.dp), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer), shape = MaterialTheme.shapes.extraLarge) {
          Column(Modifier.padding(vertical = 8.dp)) {
            ListItem(
              colors = clearItem(),
              leadingContent = { AppTile(g.id, 46.dp) },
              headlineContent = { Text(g.name, style = MaterialTheme.typography.titleMedium) },
              supportingContent = {
                Column {
                  Row {
                    Text("${plural(g.records, "item", "items")} · ${size(g.bytes)} · ")
                    if (g.installed) Text("Connected", color = Tint.ok) else Text("Not connected, data kept", color = Tint.waiting)
                  }
                  storageNote(g).takeIf { it.isNotEmpty() }?.let { Text(it) }
                }
              },
              trailingContent = {
                IconButton(onClick = {
                  shell.act {
                    val data = JSONObject()
                    for (c in g.cs) data.put(if (c.optString("id") == g.id) g.name else c.optString("name"), Core.call("export", params("container" to c.optString("id"))))
                    copy(ctx, data.toString(2))
                    shell.toast("Copied all ${g.name} data as JSON")
                  }
                }) { Icon(Icons.Rounded.ContentCopy, "Copy as JSON") }
              },
            )
            if (g.parts.isNotEmpty()) {
              Heading("In ${g.name}")
              g.parts.forEach { part ->
                ListItem(
                  colors = clearItem(),
                  headlineContent = { Text(part.optString("name")) },
                  supportingContent = {
                    Column {
                      Text(if (part.optBoolean("here")) plural(part.optInt("records"), "item", "items") else "Not on this device")
                      sharedNote(part)?.let {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                          Icon(Icons.Rounded.Group, null, Modifier.size(14.dp))
                          Text("  $it")
                        }
                      }
                    }
                  },
                )
              }
            }
            Heading("Who can access it")
            if (g.access.isEmpty()) Caption("Nobody — only you, from this screen.", Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
            g.access.forEach { a ->
              val client = a.optString("client")
              val owner = client == "app:${g.id}"
              ListItem(
                colors = clearItem(),
                leadingContent = { Dot(true) },
                headlineContent = { Text(a.optString("client_name")) },
                supportingContent = { Text((if (a.optString("access") == "write") "Read & write" else "Read only") + if (owner) " · owner" else "") },
                trailingContent = {
                  if (!owner) IconButton(onClick = {
                    shell.act {
                      Core.call("set_grant", params("client" to client, "app" to g.id, "access" to null))
                      shell.toast("${a.optString("client_name")} can no longer access ${g.name}")
                      load()
                    }
                  }) { Icon(Icons.Rounded.Close, "Remove access") }
                },
              )
            }
            if (g.copies.isNotEmpty()) {
              Heading("On devices without UUOB")
              g.copies.forEach { (id, copy) ->
                val device = copy.text("device") ?: "Another device"
                ListItem(
                  colors = clearItem(),
                  headlineContent = { Text(device) },
                  supportingContent = { Text("Connected ${day(copy.optLong("created_at"))}") },
                  trailingContent = {
                    IconButton(onClick = {
                      shell.act {
                        Core.call("delete_client", params("id" to id))
                        shell.toast("${g.name} on ${copy.text("device") ?: "that device"} was disconnected")
                        load()
                      }
                    }) { Icon(Icons.Rounded.Close, "Disconnect ${g.name} on $device") }
                  },
                )
              }
            }
            HorizontalDivider(Modifier.padding(top = 8.dp))
            TextButton(onClick = { sheet = if (g.installed) AppSheet.Disconnect(g) else AppSheet.Wipe(g) }, Modifier.padding(horizontal = 8.dp)) {
              Text(if (g.installed) "Disconnect ${g.name}…" else "Delete data…", color = MaterialTheme.colorScheme.error)
            }
          }
        }
      }
    }
  }

  when (val s = sheet) {
    AppSheet.Code -> AppCode(onClose = { sheet = null; reload() })
    is AppSheet.Disconnect -> Disconnect(s.app, onClose = { sheet = null }) { delete ->
      sheet = null
      shell.act {
        Core.call("uninstall_app", params("id" to s.app.id, "delete_data" to delete))
        shell.toast(if (delete) "${s.app.name} was disconnected and its data deleted" else "${s.app.name} was disconnected; its data is kept")
        load()
      }
    }
    is AppSheet.Wipe -> Confirm("Delete all ${s.app.name} data?",
      "${plural(s.app.records, "item is", "items are")} permanently erased from every device. This can’t be undone.", "Delete",
      onConfirm = {
        sheet = null
        shell.act { Core.call("uninstall_app", params("id" to s.app.id, "delete_data" to true)); shell.toast("${s.app.name} data deleted on all devices"); load() }
      },
      onDismiss = { sheet = null })
    null -> {}
  }
}

@Composable
private fun Heading(text: String) =
  Text(text, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 16.dp, top = 12.dp))

/** A one-time code to type into an app, also on a device without UUOB. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AppCode(onClose: () -> Unit) {
  var code by remember { mutableStateOf<JSONObject?>(null) }
  LaunchedEffect(Unit) { runCatching { code = Core.obj("create_app_code") } }
  ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      SheetTitle("Connect with a code", center = true)
      Caption("Type this code into the app. It also works in an app on a device that has no UUOB.", Modifier.fillMaxWidth().padding(top = 6.dp, bottom = 22.dp))
      code?.let { CodeView(it, "Works once.", qr = false) } ?: Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) {
        CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
      }
      Button(onClick = onClose, Modifier.fillMaxWidth().padding(top = 20.dp).height(52.dp)) { Text("Done") }
    }
  }
}

/** Disconnect an app: keep its data, or delete it too. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Disconnect(app: AppGroup, onClose: () -> Unit, onChoose: (delete: Boolean) -> Unit) {
  ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      SheetTitle("Disconnect ${app.name}?")
      Caption("${app.name} stops working with your network on all your devices. Choose what happens to its ${plural(app.records, "item", "items")}.", Modifier.padding(top = 6.dp))
      Spacer(Modifier.height(20.dp))
      Choice("Keep the data", "Connect it again later and pick up where you left off.", danger = false) { onChoose(false) }
      Spacer(Modifier.height(10.dp))
      Choice("Delete the data too", "Permanently erased from every device.", danger = true) { onChoose(true) }
      OutlinedButton(onClick = onClose, Modifier.fillMaxWidth().padding(top = 16.dp).height(52.dp)) { Text("Cancel") }
    }
  }
}

@Composable
private fun Choice(title: String, text: String, danger: Boolean, onClick: () -> Unit) =
  Card(onClick = onClick, Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)) {
    Column(Modifier.padding(16.dp)) {
      Text(title, style = MaterialTheme.typography.titleMedium, color = if (danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface)
      Caption(text)
    }
  }
