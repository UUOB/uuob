package app.uuob

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.Group
import androidx.compose.material.icons.rounded.Link
import androidx.compose.material.icons.rounded.QrCodeScanner
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
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject

private sealed interface PersonSheet {
  /** Your name first, then maybe what to open next. */
  data class Name(val then: PersonSheet?) : PersonSheet
  data object Add : PersonSheet
  data object Meet : PersonSheet
  data class Remove(val person: JSONObject) : PersonSheet
}

private fun can(access: String) = if (access == "write") "Can edit" else "Can view"

@Composable
fun PeopleScreen(shell: Shell, top: @Composable () -> Unit) {
  var me by remember { mutableStateOf("") }
  var sheet by remember { mutableStateOf<PersonSheet?>(null) }
  val people = shell.people
  LaunchedEffect(shell.tick) { runCatching { me = Core.obj("status").optJSONObject("network")?.text("name") ?: "" } }

  // Linking needs a name for the other person to see.
  fun start(what: PersonSheet) {
    sheet = if (me.isBlank()) PersonSheet.Name(what) else what
  }
  fun act(method: String, params: JSONObject, toast: String? = null) = shell.act {
    Core.call(method, params)
    toast?.let(shell::toast)
    shell.refresh()
  }

  Page("People", "Share a list or a calendar with someone who has their own UUOB. They see only what you share with them. Everything else stays private.", top) {
    if (me.isNotBlank()) block {
      Section {
        ListItem(
          colors = clearItem(),
          leadingContent = { Avatar(me, mine = true) },
          headlineContent = {
            Row(verticalAlignment = Alignment.CenterVertically) {
              Text(me)
              Surface(color = MaterialTheme.colorScheme.primaryContainer, shape = CircleShape, modifier = Modifier.padding(start = 8.dp)) {
                Text("You", Modifier.padding(horizontal = 8.dp, vertical = 2.dp), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onPrimaryContainer)
              }
            }
          },
          supportingContent = { Text("The name other people see") },
          trailingContent = { IconButton(onClick = { sheet = PersonSheet.Name(null) }) { Icon(Icons.Rounded.Edit, "Change name") } },
        )
      }
    }

    if (people.isEmpty()) block {
      Empty(Icons.Rounded.Group, "No people yet", "Link with a partner, a family member or a colleague. Then share a list or a calendar with them from Tasks or Calendar.")
    }
    people.forEach { p ->
      block(p.optString("id")) {
        val name = p.optString("name")
        Section {
          ListItem(
            colors = clearItem(),
            leadingContent = { Avatar(name) },
            headlineContent = { Text(name, style = MaterialTheme.typography.titleMedium) },
            supportingContent = {
              Row(verticalAlignment = Alignment.CenterVertically) {
                Dot(p.optBoolean("online"))
                Text("  ${if (p.optBoolean("online")) "Online" else "Offline"}")
              }
            },
            trailingContent = { IconButton(onClick = { sheet = PersonSheet.Remove(p) }) { Icon(Icons.Rounded.Delete, "Remove $name") } },
          )
          val receiving = p.objects("receiving")
          val sharing = p.objects("sharing")
          if (receiving.isNotEmpty()) {
            Label("$name shares with you")
            receiving.forEach { s ->
              ListItem(
                colors = clearItem(),
                leadingContent = { AppTile(s.optString("app"), 36.dp) },
                headlineContent = { Text(s.optString("name")) },
                supportingContent = { Text("${s.optString("app_name")} · ${can(s.optString("access"))}") },
                trailingContent = {
                  val container = s.optString("container")
                  if (s.optBoolean("offered")) Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(onClick = { act("delete_container", params("id" to container)) }) { Text("Decline") }
                    FilledTonalButton(onClick = { act("accept_share", params("container" to container), "“${s.optString("name")}” was added to ${s.optString("app_name")}") }) { Text("Accept") }
                  } else TextButton(onClick = { act("delete_container", params("id" to container), "You left “${s.optString("name")}”") }) {
                    Text("Leave", color = MaterialTheme.colorScheme.error)
                  }
                },
              )
            }
          }
          if (sharing.isNotEmpty()) {
            Label("You share with $name")
            sharing.forEach { s ->
              val container = s.optString("container")
              ListItem(
                colors = clearItem(),
                leadingContent = { AppTile(s.optString("app"), 36.dp) },
                headlineContent = { Text(s.optString("name")) },
                supportingContent = { Text(s.optString("app_name")) },
                trailingContent = {
                  IconButton(onClick = {
                    act("share_with", params("container" to container, "person" to p.optString("id"), "access" to null), "Stopped sharing “${s.optString("name")}” with $name")
                  }) { Icon(Icons.Rounded.Close, "Stop sharing") }
                },
              )
              Choice(listOf("read" to "Can view", "write" to "Can edit"), s.optString("access"), Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp)) { v ->
                act("share_with", params("container" to container, "person" to p.optString("id"), "access" to v))
              }
            }
          }
          if (receiving.isEmpty() && sharing.isEmpty()) {
            Caption("Nothing shared yet. Open a list in Tasks or a calendar in Calendar and choose Share.", Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp))
          }
        }
      }
    }

    block {
      Column(Modifier.padding(top = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Button(onClick = { start(PersonSheet.Add) }, Modifier.fillMaxWidth().height(56.dp)) {
          Icon(Icons.Rounded.Add, null)
          Text("  Add a person")
        }
        FilledTonalButton(onClick = { start(PersonSheet.Meet) }, Modifier.fillMaxWidth().height(56.dp)) {
          Icon(Icons.Rounded.Link, null)
          Text("  Enter a code")
        }
      }
    }
  }

  when (val s = sheet) {
    is PersonSheet.Name -> NameSheet(me, continues = s.then != null, onClose = { sheet = null }) { name ->
      shell.act {
        Core.call("set_name", params("name" to name))
        me = name.trim()
        sheet = s.then
      }
    }
    PersonSheet.Add -> AddPerson(shell, onClose = { sheet = null })
    PersonSheet.Meet -> Meet(shell, onClose = { sheet = null })
    is PersonSheet.Remove -> {
      val name = s.person.optString("name")
      Confirm("Remove $name?",
        "Everything you share with each other stops. What $name shared disappears from your devices, and what you shared disappears from theirs the next time they connect.",
        "Remove",
        onConfirm = { sheet = null; act("remove_person", params("id" to s.person.optString("id")), "$name was removed") },
        onDismiss = { sheet = null })
    }
    null -> {}
  }
}

@Composable
private fun Label(text: String) =
  Text(text, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp))

/** "Your name": how you appear to people you share with. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NameSheet(current: String, continues: Boolean, onClose: () -> Unit, onSave: (String) -> Unit) {
  var name by remember { mutableStateOf(current) }
  ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      SheetTitle("Your name")
      Caption("This is how you appear to people you share with.", Modifier.padding(top = 6.dp))
      OutlinedTextField(name, { name = it }, Modifier.fillMaxWidth().padding(top = 18.dp), placeholder = { Text("e.g. Alex") }, singleLine = true)
      Row(Modifier.fillMaxWidth().padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedButton(onClick = onClose, Modifier.weight(1f).height(52.dp)) { Text("Cancel") }
        Button(onClick = { onSave(name) }, Modifier.weight(1f).height(52.dp), enabled = name.isNotBlank()) { Text(if (continues) "Continue" else "Save") }
      }
    }
  }
}

/** A code for someone to link with you. The new person in the list is the one who used it. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddPerson(shell: Shell, onClose: () -> Unit) {
  var invite by remember { mutableStateOf<JSONObject?>(null) }
  var error by remember { mutableStateOf<String?>(null) }
  val before = remember { shell.people.map { it.optString("id") }.toSet() }
  val linked = shell.people.firstOrNull { invite != null && it.optString("id") !in before }?.optString("name")
  val scope = rememberCoroutineScope()
  LaunchedEffect(Unit) {
    try {
      invite = Core.obj("create_person_invite")
    } catch (e: Core.Failed) {
      error = e.message
    }
  }
  fun close() {
    val code = invite?.optString("code")
    if (code != null && linked == null) scope.launch { runCatching { Core.call("cancel_invite", params("code" to code)) } }
    onClose()
  }
  ModalBottomSheet(onDismissRequest = ::close, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      when {
        linked != null -> Success("Linked with $linked", "Nothing is shared yet. Open a list in Tasks or a calendar in Calendar and choose Share.", ::close)
        invite != null -> {
          SheetTitle("Add a person", center = true)
          Caption("They open UUOB on their device, go to People → Enter a code and type this.", Modifier.fillMaxWidth().padding(top = 6.dp, bottom = 22.dp))
          CodeView(invite!!, "Waiting for them…")
          Caption("This links the two of you. It doesn’t share anything, and it doesn’t add them to your devices.", Modifier.padding(top = 16.dp))
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

/** Type or scan the code someone else shows. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Meet(shell: Shell, onClose: () -> Unit) {
  var code by remember { mutableStateOf("") }
  var busy by remember { mutableStateOf(false) }
  var error by remember { mutableStateOf<String?>(null) }
  var scanning by remember { mutableStateOf(false) }
  val scope = rememberCoroutineScope()
  fun meet() = scope.launch {
    busy = true
    error = null
    try {
      Core.call("meet", params("code" to code))
      shell.refresh()
      shell.toast("Linked with ${shell.people.lastOrNull()?.optString("name") ?: "them"}")
      onClose()
    } catch (e: Core.Failed) {
      error = e.message
    }
    busy = false
  }
  ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      SheetTitle("Enter a code")
      Caption("Ask the other person to open People → Add a person in their UUOB (or to share something with someone new), and type the code they see.", Modifier.padding(top = 6.dp))
      Button(onClick = { scanning = true }, Modifier.fillMaxWidth().padding(top = 18.dp).height(56.dp)) {
        Icon(Icons.Rounded.QrCodeScanner, null)
        Text("  Scan QR code")
      }
      Caption("or type the code", Modifier.align(Alignment.CenterHorizontally).padding(vertical = 12.dp))
      CodeField(code, { code = it }, ::meet)
      error?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(top = 10.dp)) }
      Caption("Your devices and data stay your own. You see only what they share with you.", Modifier.padding(top = 14.dp))
      Row(Modifier.fillMaxWidth().padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedButton(onClick = onClose, Modifier.weight(1f).height(52.dp)) { Text("Cancel") }
        Button(onClick = { meet() }, Modifier.weight(1f).height(52.dp), enabled = !busy && complete(code)) { Text(if (busy) "Linking…" else "Link") }
      }
    }
  }
  if (scanning) Scanner { text, why ->
    scanning = false
    if (text != null) {
      code = text
      meet()
    } else if (why != null) error = why
  }
}
