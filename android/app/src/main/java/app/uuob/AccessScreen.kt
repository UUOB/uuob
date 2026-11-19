package app.uuob

import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Group
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.Widgets
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.background
import org.json.JSONObject

/** Which app, person or AI the Access tab has open: its `key` in `access`, e.g. "app:tasks". */
@Composable
fun AccessTab(shell: Shell, top: @Composable () -> Unit) {
  var open by rememberSaveable { mutableStateOf<String?>(null) }
  val close = { open = null }
  val key = open
  when {
    key == null -> AccessScreen(shell, top) { open = it }
    key.startsWith("app:") -> AppsScreen(shell, top, key.removePrefix("app:"), close)
    key.startsWith("ai:") -> AiScreen(shell, top, key.removePrefix("ai:"), close)
    key.startsWith("person:") -> PeopleScreen(shell, top, key.removePrefix("person:"), close)
  }
}

private enum class Show(val label: String, val kind: String?) { All("All", null), Apps("Apps", "app"), People("People", "person"), Ai("AI", "ai") }

private sealed interface AddSheet {
  data object Choose : AddSheet
  data object App : AddSheet
  data object Person : AddSheet
  data object Ai : AddSheet
}

/** Access: every app, person and AI that can reach your data, and what each can read or write. */
@Composable
private fun AccessScreen(shell: Shell, top: @Composable () -> Unit, onOpen: (String) -> Unit) {
  var rows by remember { mutableStateOf(listOf<JSONObject>()) }
  var show by rememberSaveable { mutableStateOf(Show.All) }
  var sheet by remember { mutableStateOf<AddSheet?>(null) }
  LaunchedEffect(shell.tick, shell.people) { runCatching { rows = Core.list("access") } }
  val shown = rows.filter { show.kind == null || it.optString("kind") == show.kind }

  Page("Access", "Everything that can reach your data, and what each one can read or write.", top) {
    block {
      Row(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Show.entries.forEach { s -> FilterChip(selected = show == s, onClick = { show = s }, label = { Text(s.label) }) }
        Spacer(Modifier.weight(1f))
        Button(onClick = { sheet = AddSheet.Choose }) {
          Icon(Icons.Rounded.Add, null)
          Text(" Add")
        }
      }
    }
    if (rows.isEmpty()) block {
      Empty(Icons.Rounded.Widgets, "Nothing has access yet", "Install an app like Tasks or Calendar and open it: it asks to connect. You can also link with a person or connect an AI with Add.")
    } else if (shown.isNotEmpty()) block {
      Section {
        shown.forEachIndexed { i, r ->
          if (i > 0) HorizontalDivider(Modifier.padding(start = 72.dp))
          ListItem(
            modifier = Modifier.clickable { onOpen(r.optString("key")) },
            colors = clearItem(),
            leadingContent = { RowIcon(r) },
            headlineContent = {
              Row(verticalAlignment = Alignment.CenterVertically) {
                Text(r.optString("name"), fontWeight = FontWeight.SemiBold)
                Badge(r.optString("kind"))
              }
            },
            supportingContent = {
              Column {
                Text(r.optString("summary"))
                r.text("where")?.takeIf { it.isNotEmpty() }?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
              }
            },
            trailingContent = { Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null) },
          )
        }
      }
    }
  }

  when (sheet) {
    AddSheet.Choose -> AddChooser(onClose = { sheet = null }) { sheet = it }
    AddSheet.App -> AppCode(onClose = { sheet = null; shell.tick++ })
    AddSheet.Person -> AddPersonFlow(shell, onClose = { sheet = null })
    AddSheet.Ai -> Info(
      "Connect an AI on a computer",
      "AI assistants like Claude run on a computer. Open UUOB there and choose Access → Add → An AI. You can change what it may read and write here, and it applies on all your devices.",
      onClose = { sheet = null },
    )
    null -> {}
  }
}

/** An app's tile, a person's face or the AI sparkle. */
@Composable
private fun RowIcon(r: JSONObject) {
  when (r.optString("kind")) {
    "app" -> AppTile(r.optString("id"), 40.dp)
    "person" -> Avatar(r.optString("name"), 40.dp)
    else -> Box(Modifier.size(40.dp).clip(RoundedCornerShape(11.dp)).background(Tint.apps[4]), contentAlignment = Alignment.Center) {
      Icon(Icons.Rounded.AutoAwesome, null, tint = Color.White)
    }
  }
}

/** App, Person or AI, small next to the name. */
@Composable
private fun Badge(kind: String) {
  val (text, color) = when (kind) {
    "app" -> "App" to Tint.apps[0]
    "person" -> "Person" to Tint.apps[6]
    else -> "AI" to Tint.apps[4]
  }
  Surface(color = color.copy(alpha = 0.18f), contentColor = MaterialTheme.colorScheme.onSurface, shape = RoundedCornerShape(6.dp), modifier = Modifier.padding(start = 8.dp)) {
    Text(text.uppercase(), Modifier.padding(horizontal = 7.dp, vertical = 2.dp), style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Bold)
  }
}

/** "What are you adding?": an app, a person or an AI. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddChooser(onClose: () -> Unit, onChoose: (AddSheet) -> Unit) {
  ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      SheetTitle("What are you adding?")
      Spacer(Modifier.height(16.dp))
      Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Kind(Icons.Rounded.Widgets, Tint.apps[0], "An app", "An app on one of your devices, or on a device without UUOB.") { onChoose(AddSheet.App) }
        Kind(Icons.Rounded.Person, Tint.apps[6], "A person", "Someone with their own UUOB. You choose what to share.") { onChoose(AddSheet.Person) }
        Kind(Icons.Rounded.AutoAwesome, Tint.apps[4], "An AI", "An assistant that sees only what you pick.") { onChoose(AddSheet.Ai) }
      }
      OutlinedButton(onClick = onClose, Modifier.fillMaxWidth().padding(top = 16.dp).height(52.dp)) { Text("Cancel") }
    }
  }
}

@Composable
private fun Kind(icon: androidx.compose.ui.graphics.vector.ImageVector, color: Color, title: String, text: String, onClick: () -> Unit) =
  Card(onClick = onClick, Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)) {
    Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
      Box(Modifier.size(44.dp).clip(if (title == "A person") CircleShape else RoundedCornerShape(12.dp)).background(color), contentAlignment = Alignment.Center) {
        Icon(icon, null, tint = Color.White)
      }
      Column(Modifier.padding(start = 14.dp)) {
        Text(title, style = MaterialTheme.typography.titleMedium)
        Caption(text)
      }
    }
  }

/** A title, a paragraph and Done. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Info(title: String, text: String, onClose: () -> Unit) {
  ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)) {
    SheetBody {
      SheetTitle(title)
      Caption(text, Modifier.padding(top = 6.dp))
      Button(onClick = onClose, Modifier.fillMaxWidth().padding(top = 20.dp).height(52.dp)) { Text("Done") }
    }
  }
}
