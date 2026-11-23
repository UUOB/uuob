package app.uuob

import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.background
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
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteDefaults
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
import androidx.compose.runtime.remember
import androidx.compose.ui.input.key.Key
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
  /** Add was asked for with the keyboard (Ctrl+N): Access opens its chooser. */
  var adding by mutableStateOf(false)
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

private enum class Screen(val title: String, val icon: String) {
  Network("Network", "network"),
  Access("Access", "shield"),
  Settings("Settings", "settings"),
}

@Composable
fun UuobApp(shell: Shell) {
  var screen by rememberSaveable { mutableStateOf(Screen.Network) }
  LaunchedEffect(Unit) { shell.follow() }

  val scheme = MaterialTheme.colorScheme
  // The keys every UUOB app answers to, with a keyboard: Ctrl+, Settings,
  // Ctrl+N Add (in Access), Ctrl+R Refresh, Ctrl+1–3 the tabs, Ctrl+/ the list.
  val keys = remember {
    app.uuob.kit.Shortcuts(
      settings = { screen = Screen.Settings },
      new = "Add" to { screen = Screen.Access; shell.adding = true },
      refresh = { shell.tick++ },
      extra = Screen.entries.mapIndexed { i, s ->
        app.uuob.kit.Shortcuts.Extra(s.title, "Ctrl+${i + 1}", listOf(Key.One, Key.Two, Key.Three)[i], ctrl = true) { screen = s }
      },
    )
  }
  app.uuob.kit.ShortcutsHost(keys) {
  Column(Modifier.fillMaxSize().background(scheme.background)) {
    Box(Modifier.weight(1f)) {
      Column(Modifier.fillMaxSize()) {
        shell.serverError?.let {
          Surface(color = MaterialTheme.colorScheme.errorContainer, contentColor = MaterialTheme.colorScheme.onErrorContainer, modifier = Modifier.fillMaxWidth()) {
            Text("$it Close other copies of UUOB and open it again.", Modifier.padding(12.dp), textAlign = TextAlign.Center)
          }
        }
        val top = @Composable {}
        when (screen) {
          Screen.Network -> NetworkScreen(shell, top)
          Screen.Access -> AccessTab(shell, top)
          Screen.Settings -> SettingsScreen(shell, top)
        }
      }
      // Toasts: a small pill, as UUOB always showed them.
      SnackbarHost(shell.snackbar, Modifier.align(Alignment.BottomCenter).padding(16.dp)) { data ->
        Surface(
          shape = androidx.compose.foundation.shape.CircleShape, color = Look.elevated, contentColor = scheme.onSurface,
          border = androidx.compose.foundation.BorderStroke(1.dp, Look.lineStrong), shadowElevation = 8.dp,
        ) {
          Text(data.visuals.message, Modifier.padding(horizontal = 20.dp, vertical = 12.dp), style = MaterialTheme.typography.bodyMedium, fontWeight = androidx.compose.ui.text.font.FontWeight.Medium)
        }
      }
    }
    // The tab bar, as UUOB's rail became on a phone: a hairline above, each
    // tab an icon in a soft tile with its name under it, a dot for news.
    Line()
    Row(Modifier.fillMaxWidth().background(scheme.surfaceContainerLow).navigationBarsPadding().padding(4.dp)) {
      Screen.entries.forEach { s ->
        val on = screen == s
        val st = shell.status
        val dot = when {
          s == Screen.Network && (st?.optInt("devices_total") ?: 0) > 1 -> if ((st?.optInt("devices_online") ?: 0) > 1) Tint.ok else Look.text3
          s == Screen.Access && shell.asks.size + shell.offers.size > 0 -> Tint.waiting
          else -> null
        }
        Column(
          Modifier.weight(1f).clip(RoundedCornerShape(14.dp))
            .selectable(selected = on, role = androidx.compose.ui.semantics.Role.Tab) { screen = s }
            .padding(vertical = 6.dp),
          horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
          Box(Modifier.size(40.dp).clip(RoundedCornerShape(12.dp)).background(if (on) Look.active else Color.Transparent), contentAlignment = Alignment.Center) {
            Ico(s.icon, null, tint = if (on) scheme.onSurface else Look.text2)
            if (dot != null) Box(
              Modifier.align(Alignment.TopEnd).padding(top = 6.dp, end = 6.dp).size(12.dp).clip(CircleShape)
                .background(Look.elevated).padding(2.dp).clip(CircleShape).background(dot),
            )
          }
          Text(s.title, style = androidx.compose.ui.text.TextStyle(fontSize = 11.sp, fontWeight = FontWeight.SemiBold), color = if (on) scheme.onSurface else Look.text2)
        }
      }
    }
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
  Sheet(onDismissRequest = { answer(false) }, sheetState = state, dragHandle = null) {
    SheetBody {
      Spacer(Modifier.height(32.dp))
      Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
        AppTile(ask.optString("app"), 58.dp)
        Ico("right", null, Modifier.padding(horizontal = 12.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
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
        UTonal(onClick = { answer(false) }, Modifier.weight(1f), large = true) { Text("Don’t allow") }
        UButton(onClick = { answer(true) }, Modifier.weight(1f), large = true) { Text("Allow") }
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
  Sheet(onDismissRequest = { shell.later += container }, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
        Avatar(person.optString("name"), 58.dp)
        Ico("right", null, Modifier.padding(horizontal = 12.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
        AppTile(offer.optString("app"), 58.dp)
      }
      Spacer(Modifier.height(18.dp))
      SheetTitle("${person.optString("name")} shared “${offer.optString("name")}” with you", center = true)
      val can = if (offer.optString("access") == "write") "You can both change it." else "You can view it, but not change it."
      Caption("It will appear in ${offer.optString("app_name")} on all your devices. $can", Modifier.fillMaxWidth().padding(top = 6.dp))
      Row(Modifier.fillMaxWidth().padding(top = 24.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        UTonal(onClick = { answer(false) }, Modifier.weight(1f), large = true) { Text("Decline") }
        UButton(onClick = { answer(true) }, Modifier.weight(1f), large = true) { Text("Accept") }
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
