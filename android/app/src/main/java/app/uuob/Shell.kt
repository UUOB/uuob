package app.uuob

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.Hub
import androidx.compose.material.icons.rounded.Shield
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SheetValue
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteScaffold
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import org.json.JSONObject

/** What the whole app shows: the network, questions from apps, offers from people, toasts. */
class Shell(private val scope: CoroutineScope) {
  var status by mutableStateOf<JSONObject?>(null)
  var people by mutableStateOf<List<JSONObject>>(emptyList())
  /** Questions from apps, answered here. */
  var asks by mutableStateOf<List<JSONObject>>(emptyList())
  /** Offers put aside until UUOB is opened again. */
  var later by mutableStateOf(setOf<String>())
  var serverError by mutableStateOf<String?>(null)
  /** Bumped on network changes, so screens reload. */
  var tick by mutableIntStateOf(0)
  val snackbar = SnackbarHostState()

  fun toast(text: String) {
    scope.launch {
      snackbar.currentSnackbarData?.dismiss()
      snackbar.showSnackbar(text)
    }
  }

  /** Runs `block`; a failure shows as a toast. */
  fun act(block: suspend () -> Unit) {
    scope.launch {
      try {
        block()
      } catch (e: Core.Failed) {
        toast(e.message ?: "Something went wrong")
      }
    }
  }

  suspend fun refresh() {
    try {
      status = Core.obj("status")
      people = Core.list("people")
      serverError = Core.call("server_error") as? String
    } catch (_: Core.Failed) {}
  }

  suspend fun loadAsks() {
    try {
      asks = Core.list("asks")
    } catch (_: Core.Failed) {}
  }

  /** Follows the node's changes for as long as the screen is there. */
  suspend fun follow() {
    refresh()
    loadAsks()
    Core.events.collect { e ->
      when (e.optString("type")) {
        "container_changed" -> return@collect
        "asks_changed" -> return@collect loadAsks()
        "device_joined" -> toast("${e.optString("name")} joined your network")
        "removed_from_network" -> toast("This device was removed from the network")
        "lagged" -> loadAsks()
      }
      refresh()
      tick++
    }
  }

  /** Things other people offered to share, waiting for an answer. */
  val offers get() = people.flatMap { p -> p.objects("receiving").filter { it.optBoolean("offered") }.map { it to p } }
}

private enum class Screen(val title: String, val icon: androidx.compose.ui.graphics.vector.ImageVector) {
  Network("Network", Icons.Rounded.Hub),
  Access("Access", Icons.Rounded.Shield),
  Settings("Settings", Icons.Rounded.Tune),
}

@Composable
fun UuobApp(shell: Shell, updater: Updater) {
  var screen by rememberSaveable { mutableStateOf(Screen.Network) }
  LaunchedEffect(Unit) { shell.follow() }

  NavigationSuiteScaffold(navigationSuiteItems = {
    Screen.entries.forEach { s ->
      item(
        selected = screen == s,
        onClick = { screen = s },
        label = { Text(s.title) },
        icon = {
          val st = shell.status
          BadgedBox(badge = {
            if (s == Screen.Network && (st?.optInt("devices_total") ?: 0) > 1) Badge(containerColor = if ((st?.optInt("devices_online") ?: 0) > 1) Tint.ok else Tint.offline)
            else if (s == Screen.Access && shell.asks.size + shell.offers.size > 0) Badge { Text("${shell.asks.size + shell.offers.size}") }
          }) { Icon(s.icon, null) }
        },
      )
    }
  }) {
    Box(Modifier.fillMaxSize()) {
      Column(Modifier.fillMaxSize()) {
        shell.serverError?.let {
          Surface(color = MaterialTheme.colorScheme.errorContainer, contentColor = MaterialTheme.colorScheme.onErrorContainer, modifier = Modifier.fillMaxWidth()) {
            Text("$it Close other copies of UUOB and open it again.", Modifier.padding(12.dp), textAlign = TextAlign.Center)
          }
        }
        val top = @Composable { UpdateButton(updater) }
        when (screen) {
          Screen.Network -> NetworkScreen(shell, top)
          Screen.Access -> AccessTab(shell, top)
          Screen.Settings -> SettingsScreen(shell, updater, top)
        }
      }
      SnackbarHost(shell.snackbar, Modifier.align(Alignment.BottomCenter).padding(16.dp))
    }
  }

  val ask = shell.asks.firstOrNull()
  val offer = shell.offers.firstOrNull { (s, _) -> s.optString("container") !in shell.later }
  when {
    ask?.optString("kind") == "share" -> ShareSheet(shell, ask)
    ask != null -> AskSheet(shell, ask)
    offer != null -> OfferSheet(shell, offer.first, offer.second)
  }
}

/** "“Tasks” wants to use your network": an app asks to connect, or for another app's data. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AskSheet(shell: Shell, ask: JSONObject) {
  val scope = rememberCoroutineScope()
  // Only an answer closes it; back means "Don't allow".
  val state = rememberModalBottomSheetState(skipPartiallyExpanded = true, confirmValueChange = { it != SheetValue.Hidden })
  fun answer(allow: Boolean) = scope.launch {
    try {
      Core.call("answer", params("id" to ask.optString("id"), "allow" to allow))
    } catch (_: Core.Failed) {}
    if (allow && ask.optString("kind") == "connect") shell.toast("${ask.optString("name")} is connected")
    shell.loadAsks()
    shell.tick++
  }
  val connect = ask.optString("kind") == "connect"
  ModalBottomSheet(onDismissRequest = { answer(false) }, sheetState = state, dragHandle = null) {
    SheetBody {
      Spacer(Modifier.height(32.dp))
      Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
        AppTile(ask.optString("app"), 58.dp)
        Icon(Icons.AutoMirrored.Rounded.ArrowForward, null, Modifier.padding(horizontal = 12.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        if (connect) Logo(58.dp) else AppTile(ask.optString("container"), 58.dp)
      }
      Spacer(Modifier.height(18.dp))
      if (connect) {
        SheetTitle("“${ask.optString("name")}” wants to use your network", center = true)
        Caption("It gets its own place for its data, synced to all your devices. It can’t see anything else unless you allow it.", Modifier.fillMaxWidth().padding(top = 6.dp))
      } else {
        val what = if (ask.optString("access") == "write") "read and change" else "read"
        SheetTitle("“${ask.optString("app_name")}” wants to $what your ${ask.optString("container_name")} data", center = true)
        Caption("You can change this any time in Access.", Modifier.fillMaxWidth().padding(top = 6.dp))
      }
      Row(Modifier.fillMaxWidth().padding(top = 24.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedButton(onClick = { answer(false) }, Modifier.weight(1f).height(52.dp)) { Text("Don’t allow") }
        Button(onClick = { answer(true) }, Modifier.weight(1f).height(52.dp)) { Text("Allow") }
      }
    }
  }
}

/** Someone shared a list or a calendar with you. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun OfferSheet(shell: Shell, offer: JSONObject, person: JSONObject) {
  val container = offer.optString("container")
  fun answer(accept: Boolean) = shell.act {
    shell.later += container
    if (accept) Core.call("accept_share", params("container" to container)) else Core.call("delete_container", params("id" to container))
    if (accept) shell.toast("“${offer.optString("name")}” was added to ${offer.optString("app_name")}")
    shell.refresh()
    shell.tick++
  }
  ModalBottomSheet(onDismissRequest = { shell.later += container }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
        Avatar(person.optString("name"), 58.dp)
        Icon(Icons.AutoMirrored.Rounded.ArrowForward, null, Modifier.padding(horizontal = 12.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        AppTile(offer.optString("app"), 58.dp)
      }
      Spacer(Modifier.height(18.dp))
      SheetTitle("${person.optString("name")} shared “${offer.optString("name")}” with you", center = true)
      val can = if (offer.optString("access") == "write") "You can both change it." else "You can view it, but not change it."
      Caption("It will appear in ${offer.optString("app_name")} on all your devices. $can", Modifier.fillMaxWidth().padding(top = 6.dp))
      Row(Modifier.fillMaxWidth().padding(top = 24.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedButton(onClick = { answer(false) }, Modifier.weight(1f).height(52.dp)) { Text("Decline") }
        Button(onClick = { answer(true) }, Modifier.weight(1f).height(52.dp)) { Text("Accept") }
      }
    }
  }
}

/** UUOB's logo. */
@Composable
fun Logo(size: androidx.compose.ui.unit.Dp) =
  androidx.compose.foundation.Image(
    androidx.compose.ui.res.painterResource(R.mipmap.ic_launcher_round), null,
    Modifier.size(size),
  )
