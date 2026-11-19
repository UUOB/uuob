package app.uuob

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.Laptop
import androidx.compose.material.icons.rounded.Link
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject

private sealed interface DeviceSheet {
  data object Add : DeviceSheet
  data object Join : DeviceSheet
  data object Rename : DeviceSheet
  data class Remove(val device: JSONObject) : DeviceSheet
}

private fun platformName(p: String) = mapOf("linux" to "Linux", "macos" to "macOS", "windows" to "Windows", "android" to "Android", "ios" to "iOS")[p] ?: p

/** The devices, loaded for a tab and reloaded on every change. */
@Composable
private fun rememberDevices(shell: Shell): Pair<List<JSONObject>, suspend () -> Unit> {
  var devices by remember { mutableStateOf(listOf<JSONObject>()) }
  val load: suspend () -> Unit = { devices = Core.list("devices") }
  LaunchedEffect(shell.tick) { runCatching { load() } }
  return devices to load
}

/** Network: the graph of who is where, then your devices. */
@Composable
fun NetworkScreen(shell: Shell, top: @Composable () -> Unit) {
  val (devices, load) = rememberDevices(shell)
  var sheet by remember { mutableStateOf<DeviceSheet?>(null) }
  fun reload() = shell.act { load() }
  val alone = devices.size <= 1
  val me = devices.firstOrNull { it.optBoolean("is_self") }

  Page("Network", "Your devices in the middle, each app on the device it runs on, people at the edge.", top) {
    block { Graph(shell, devices) }
    block {
      Section("Devices") {
        devices.forEachIndexed { i, d ->
          if (i > 0) HorizontalDivider(Modifier.padding(start = 76.dp))
          val self = d.optBoolean("is_self")
          ListItem(
            colors = clearItem(),
            leadingContent = {
              Surface(shape = MaterialTheme.shapes.medium, color = MaterialTheme.colorScheme.surfaceContainerHigh, modifier = Modifier.size(44.dp)) {
                Box(contentAlignment = Alignment.Center) {
                  Icon(if (d.optString("platform") in setOf("android", "ios")) Icons.Rounded.PhoneAndroid else Icons.Rounded.Laptop, null)
                }
              }
            },
            headlineContent = { Text(d.optString("name")) },
            supportingContent = {
              Row(verticalAlignment = Alignment.CenterVertically) {
                Dot(d.optBoolean("online"))
                val keeps = if (d.optBoolean("keeper")) " · keeps everything" else ""
                Text("  ${if (self) "This device" else if (d.optBoolean("online")) "Online" else "Offline"} · ${platformName(d.optString("platform"))}$keeps")
              }
            },
            trailingContent = {
              if (self) IconButton(onClick = { sheet = DeviceSheet.Rename }) { Icon(Icons.Rounded.Edit, "Rename") }
              else IconButton(onClick = { sheet = DeviceSheet.Remove(d) }) { Icon(Icons.Rounded.Delete, "Remove from network") }
            },
          )
        }
      }
    }
    block {
      Column(Modifier.padding(top = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Button(onClick = { sheet = DeviceSheet.Add }, Modifier.fillMaxWidth().height(56.dp)) {
          Icon(Icons.Rounded.Add, null)
          Text("  Add a device")
        }
        if (alone) FilledTonalButton(onClick = { sheet = DeviceSheet.Join }, Modifier.fillMaxWidth().height(56.dp)) {
          Icon(Icons.Rounded.Link, null)
          Text("  Join a network")
        }
      }
    }
  }

  when (val s = sheet) {
    DeviceSheet.Add -> AddDevice(onClose = { sheet = null; reload() })
    DeviceSheet.Join -> JoinNetwork(shell, onClose = { sheet = null; reload() })
    DeviceSheet.Rename -> Rename(me?.optString("name") ?: "", onClose = { sheet = null }) { name ->
      shell.act { Core.call("set_device_name", params("name" to name)); sheet = null; load(); shell.tick++ }
    }
    is DeviceSheet.Remove -> Confirm("Remove “${s.device.optString("name")}”?",
      "It stops receiving updates right away. Data it already has stays on it. To use it again, add it back with a new code.", "Remove",
      onConfirm = {
        sheet = null
        shell.act { Core.call("remove_device", params("id" to s.device.optString("id"))); shell.toast("${s.device.optString("name")} was removed"); load(); shell.tick++ }
      },
      onDismiss = { sheet = null })
    else -> {}
  }
}

/** Settings: your name, this phone, storage, updates, leaving the network. */
@Composable
fun SettingsScreen(shell: Shell, updater: Updater, top: @Composable () -> Unit) {
  val ctx = LocalContext.current
  val (devices, loadDevices) = rememberDevices(shell)
  var copies by remember { mutableIntStateOf(1) }
  var me by remember { mutableStateOf("") }
  var version by remember { mutableStateOf("") }
  var atBoot by remember { mutableStateOf(BootReceiver.isOn(ctx)) }
  var leaving by remember { mutableStateOf(false) }
  var naming by remember { mutableStateOf(false) }
  suspend fun load() {
    loadDevices()
    val network = Core.obj("status").optJSONObject("network")
    copies = network?.optInt("copies", 1) ?: 1
    me = network?.text("name") ?: ""
    version = Core.call("version") as? String ?: ""
  }
  LaunchedEffect(shell.tick) { runCatching { load() } }
  val alone = devices.size <= 1
  val keepers = devices.count { it.optBoolean("keeper") }

  Page("Settings", "This phone, your storage and updates.", top) {
    block {
      Section("You") {
        ListItem(
          colors = clearItem(),
          leadingContent = { Avatar(me.ifBlank { "?" }, mine = true) },
          headlineContent = { Text(me.ifBlank { "No name yet" }) },
          supportingContent = { Text("The name people you link with see") },
          trailingContent = { IconButton(onClick = { naming = true }) { Icon(Icons.Rounded.Edit, "Change name") } },
        )
      }
    }
    block {
      Section("This phone") {
        ListItem(
          colors = clearItem(),
          headlineContent = { Text("Start when the phone turns on") },
          supportingContent = { Text("UUOB starts in the background once you unlock the phone, so your apps and devices stay in sync.") },
          trailingContent = { Switch(atBoot, onCheckedChange = { BootReceiver.set(ctx, it); atBoot = BootReceiver.isOn(ctx) }) },
        )
        HorizontalDivider(Modifier.padding(horizontal = 16.dp))
        ListItem(
          colors = clearItem(),
          headlineContent = { Text("Apps can connect") },
          supportingContent = { Text(shell.serverError ?: "Only apps on this phone that you allow in Access.") },
          trailingContent = {
            val ok = shell.serverError == null
            Text(if (ok) "Running" else "Stopped", color = if (ok) Tint.ok else MaterialTheme.colorScheme.error)
          },
        )
      }
    }
    if (!alone) block {
      val note = if (keepers > 0) {
        val who = if (copies == 1) "a storage device has" else "$copies storage devices have"
        "Apps with large data, like photos, can keep less on your other devices. Nothing is removed from a device until $who it."
      } else "Turn this on for a device with plenty of space that is usually on. Apps with large data, like photos, can then keep less on your other devices."
      Section("Storage devices", note) {
        devices.forEach { d ->
          val keeper = d.optBoolean("keeper")
          ListItem(
            colors = clearItem(),
            headlineContent = { Text(d.optString("name")) },
            supportingContent = { Text(if (keeper) "Keeps everything" else "Keeps what its apps ask for") },
            trailingContent = {
              Switch(keeper, onCheckedChange = { on -> shell.act { Core.call("set_keeper", params("device" to d.optString("id"), "on" to on)); load(); shell.tick++ } })
            },
          )
        }
        if (keepers > 0) ListItem(
          colors = clearItem(),
          headlineContent = { Text("Copies") },
          supportingContent = { Text("Storage devices that must have something before another device frees its own copy.") },
          trailingContent = {
            Choice(listOf("1" to "1", "2" to "2"), copies.toString()) { n -> shell.act { Core.call("set_copies", params("copies" to n.toInt())); load() } }
          },
        )
      }
    }
    block {
      Section("Updates") {
        ListItem(
          colors = clearItem(),
          headlineContent = { Text("UUOB $version") },
          supportingContent = { Text(updater.version?.let { "Version $it is ready" } ?: "Up to date") },
          trailingContent = { UpdateButton(updater) },
        )
      }
    }
    if (!alone) block {
      Section("Network") {
        ListItem(
          colors = clearItem(),
          headlineContent = { Text("Leave this network") },
          supportingContent = { Text("This phone stops syncing with your other devices.") },
          trailingContent = { TextButton(onClick = { leaving = true }) { Text("Leave", color = MaterialTheme.colorScheme.error) } },
        )
      }
    }
  }

  if (naming) NameSheet(me, continues = false, onClose = { naming = false }) { name ->
    shell.act { Core.call("set_name", params("name" to name)); me = name.trim(); naming = false }
  }
  if (leaving) Confirm("Leave this network?", "This device stops syncing with your other devices. Everything already on it stays here.", "Leave",
    onConfirm = { leaving = false; shell.act { Core.call("leave_network"); shell.toast("This device now has its own network"); load(); shell.tick++ } },
    onDismiss = { leaving = false })
}

@Composable
private fun Rename(current: String, onClose: () -> Unit, onSave: (String) -> Unit) {
  var name by remember { mutableStateOf(current) }
  AlertDialog(
    onDismissRequest = onClose,
    title = { Text("This device’s name") },
    text = { OutlinedTextField(name, { name = it }, singleLine = true) },
    confirmButton = { Button(onClick = { onSave(name) }, enabled = name.isNotBlank()) { Text("Save") } },
    dismissButton = { TextButton(onClick = onClose) { Text("Cancel") } },
  )
}

/** A code (and QR code) for a new device, until it joins. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddDevice(onClose: () -> Unit) {
  var invite by remember { mutableStateOf<JSONObject?>(null) }
  var joined by remember { mutableStateOf<String?>(null) }
  var error by remember { mutableStateOf<String?>(null) }
  val scope = rememberCoroutineScope()
  LaunchedEffect(Unit) {
    try {
      invite = Core.obj("create_invite")
    } catch (e: Core.Failed) {
      error = e.message
    }
  }
  LaunchedEffect(Unit) { Core.events.collect { if (it.optString("type") == "device_joined") joined = it.optString("name") } }
  fun close() {
    val code = invite?.optString("code")
    if (code != null && joined == null) scope.launch { runCatching { Core.call("cancel_invite", params("code" to code)) } }
    onClose()
  }
  ModalBottomSheet(onDismissRequest = ::close, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      when {
        joined != null -> Success("$joined joined", "Your apps and data are syncing to it now.", ::close)
        invite != null -> {
          SheetTitle("Add a device", center = true)
          Caption("On the other device, open UUOB, choose Join a network and enter this code.", Modifier.fillMaxWidth().padding(top = 6.dp, bottom = 22.dp))
          CodeView(invite!!, "Waiting for your device…")
          OutlinedButton(onClick = ::close, Modifier.fillMaxWidth().padding(top = 20.dp).height(52.dp)) { Text("Cancel") }
        }
        error != null -> {
          SheetTitle("Couldn’t create a code")
          Caption(error!!, Modifier.padding(top = 6.dp))
          OutlinedButton(onClick = ::close, Modifier.fillMaxWidth().padding(top = 20.dp)) { Text("Close") }
        }
        else -> Row(Modifier.fillMaxWidth().padding(24.dp), horizontalArrangement = Arrangement.Center) {
          CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
          Caption("  Creating a code…")
        }
      }
    }
  }
}

/** A green tick and what just worked. */
@Composable
fun Success(title: String, text: String, onDone: () -> Unit) {
  Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
    Surface(shape = CircleShape, color = Tint.ok, modifier = Modifier.size(76.dp)) {
      Box(contentAlignment = Alignment.Center) { Icon(Icons.Rounded.Check, null, Modifier.size(40.dp), tint = Color.White) }
    }
    Spacer(Modifier.height(16.dp))
    SheetTitle(title, center = true)
    Caption(text, Modifier.padding(top = 6.dp))
    Button(onClick = onDone, Modifier.fillMaxWidth().padding(top = 22.dp).height(52.dp)) { Text("Done") }
  }
}

/** Type or scan the code another device shows. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun JoinNetwork(shell: Shell, onClose: () -> Unit) {
  var code by remember { mutableStateOf("") }
  var busy by remember { mutableStateOf(false) }
  var error by remember { mutableStateOf<String?>(null) }
  var scanning by remember { mutableStateOf(false) }
  val scope = rememberCoroutineScope()
  fun join() = scope.launch {
    busy = true
    error = null
    try {
      Core.call("join", params("code" to code))
      shell.toast("Linked! Your data will sync in a moment.")
      onClose()
    } catch (e: Core.Failed) {
      error = e.message
    }
    busy = false
  }
  ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      SheetTitle("Join a network")
      Caption("On a device that’s already set up, open Network → Add a device and type the code it shows.", Modifier.padding(top = 6.dp))
      Button(onClick = { scanning = true }, Modifier.fillMaxWidth().padding(top = 18.dp).height(56.dp)) {
        Icon(Icons.Rounded.QrCodeScanner, null)
        Text("  Scan QR code")
      }
      Caption("or type the code", Modifier.align(Alignment.CenterHorizontally).padding(vertical = 12.dp))
      CodeField(code, { code = it }, ::join)
      error?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 10.dp)) }
      Caption("Anything already on this device is kept and merged into the network.", Modifier.padding(top = 14.dp))
      Row(Modifier.fillMaxWidth().padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedButton(onClick = onClose, Modifier.weight(1f).height(52.dp)) { Text("Cancel") }
        Button(onClick = { join() }, Modifier.weight(1f).height(52.dp), enabled = !busy && complete(code)) {
          Text(if (busy) "Linking…" else "Join", fontWeight = FontWeight.SemiBold)
        }
      }
    }
  }
  if (scanning) Scanner { text, why ->
    scanning = false
    if (text != null) {
      code = text
      join()
    } else if (why != null) error = why
  }
}
