package app.uuob

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
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

private val LEVELS = listOf("none" to "No access", "read" to "Can view", "write" to "Can edit")

/**
 * The share sheet: an app asked to share one of its lists or calendars, and
 * the user chooses here who gets it. Changes apply as they are made.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ShareSheet(shell: Shell, ask: JSONObject) {
  val container = ask.optString("container")
  var step by remember { mutableStateOf("people") } // people | name | new
  var me by remember { mutableStateOf("") }
  var access by remember { mutableStateOf("write") } // for someone new
  var invite by remember { mutableStateOf<JSONObject?>(null) }
  var error by remember { mutableStateOf<String?>(null) }
  var known by remember { mutableStateOf(shell.people.map { it.optString("id") }.toSet()) }
  val scope = rememberCoroutineScope()
  LaunchedEffect(Unit) { runCatching { me = Core.obj("status").optJSONObject("network")?.text("name") ?: "" } }

  fun cancelInvite() {
    invite?.optString("code")?.let { code -> scope.launch { runCatching { Core.call("cancel_invite", params("code" to code)) } } }
    invite = null
  }
  // The code both links the person and shares this with them.
  fun newInvite() {
    cancelInvite()
    error = null
    scope.launch {
      try {
        invite = Core.obj("create_person_invite", params("container" to container, "access" to access))
      } catch (e: Core.Failed) {
        error = e.message
      }
    }
  }
  fun someoneNew() {
    if (me.isBlank()) step = "name" else { step = "new"; newInvite() }
  }
  fun back() {
    cancelInvite()
    error = null
    step = "people"
  }
  fun done() = scope.launch {
    cancelInvite()
    runCatching { Core.call("answer", params("id" to ask.optString("id"), "allow" to true)) }
    shell.loadAsks()
    shell.tick++
  }

  // While a code is showing, a new person in the list is the one who used it.
  val joined = shell.people.firstOrNull { it.optString("id") !in known }
  LaunchedEffect(joined) {
    if (joined != null && step == "new" && invite != null) {
      invite = null
      step = "people"
      shell.toast("Linked with ${joined.optString("name")}")
    }
    known = shell.people.map { it.optString("id") }.toSet()
  }

  if (step == "name") {
    NameSheet("", continues = true, onClose = ::back) { name ->
      shell.act {
        Core.call("set_name", params("name" to name))
        me = name.trim()
        someoneNew()
      }
    }
    return
  }
  ModalBottomSheet(onDismissRequest = { if (step == "people") done() else back() }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      if (step == "new") {
        SheetTitle("Share with someone new", center = true)
        Caption("They open UUOB on their device, go to People → Enter a code and type this.", Modifier.fillMaxWidth().padding(top = 6.dp, bottom = 18.dp))
        Choice(LEVELS.drop(1), access, Modifier.align(Alignment.CenterHorizontally).padding(bottom = 22.dp)) { access = it; newInvite() }
        when {
          invite != null -> CodeView(invite!!, "Waiting for them…")
          error != null -> Text(error!!, color = MaterialTheme.colorScheme.error)
          else -> Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center) { CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp) }
        }
        OutlinedButton(onClick = ::back, Modifier.fillMaxWidth().padding(top = 20.dp).height(52.dp)) { Text("Back") }
      } else {
        Row(verticalAlignment = Alignment.CenterVertically) {
          AppTile(ask.optString("app"), 52.dp)
          Column(Modifier.padding(start = 14.dp)) {
            SheetTitle("Share “${ask.optString("container_name")}”")
            Caption("From ${ask.optString("app_name")}. People you choose get this and nothing else.")
          }
        }
        Column(Modifier.padding(vertical = 16.dp)) {
          shell.people.forEach { p ->
            val level = p.objects("sharing").firstOrNull { it.optString("container") == container }?.optString("access") ?: "none"
            ListItem(
              colors = clearItem(),
              leadingContent = { Avatar(p.optString("name"), 36.dp) },
              headlineContent = { Text(p.optString("name")) },
              supportingContent = {
                Choice(LEVELS, level, Modifier.padding(top = 8.dp)) { v ->
                  shell.act {
                    Core.call("share_with", params("container" to container, "person" to p.optString("id"), "access" to v.takeIf { it != "none" }))
                    shell.refresh()
                  }
                }
              },
            )
            HorizontalDivider()
          }
          ListItem(
            colors = clearItem(),
            modifier = Modifier.padding(top = 4.dp).clickable(onClick = ::someoneNew),
            leadingContent = { Icon(Icons.Rounded.Add, null, tint = MaterialTheme.colorScheme.primary) },
            headlineContent = { Text("Someone new…", color = MaterialTheme.colorScheme.primary) },
            trailingContent = { Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null) },
          )
        }
        Button(onClick = { done() }, Modifier.fillMaxWidth().height(52.dp)) { Text("Done") }
      }
    }
  }
}
