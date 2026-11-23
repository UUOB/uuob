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
import androidx.compose.ui.unit.sp
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
  data object ShowCode : AddSheet
  data object EnterCode : AddSheet
  data object Ai : AddSheet
}

/** Access: every app, person and AI that can reach your data, and what each can read or write. */
@Composable
private fun AccessScreen(shell: Shell, top: @Composable () -> Unit, onOpen: (String) -> Unit) {
  var rows by remember { mutableStateOf(listOf<JSONObject>()) }
  var show by rememberSaveable { mutableStateOf(Show.All) }
  var sheet by remember { mutableStateOf<AddSheet?>(null) }
  LaunchedEffect(shell.tick, shell.people) { runCatching { rows = Core.list("access") } }
  LaunchedEffect(shell.adding) { if (shell.adding) { shell.adding = false; sheet = AddSheet.Choose } }
  val shown = rows.filter { show.kind == null || it.optString("kind") == show.kind }

  Page("Access", "", top = {
    app.uuob.kit.PlusButton(onClick = { sheet = AddSheet.Choose }, description = "Add an app, a person or an AI (Ctrl+N or C)")
  }) {
    block {
      Choice(Show.entries.map { it.name to it.label }, show.name, Modifier.fillMaxWidth().padding(top = 20.dp)) { show = Show.valueOf(it) }
    }
    if (rows.isEmpty()) block {
      app.uuob.kit.EmptyState("shield", "Nothing has access yet", action = "Add" to { sheet = AddSheet.Choose })
    } else if (shown.isNotEmpty()) block {
      Section {
        shown.forEachIndexed { i, r ->
          if (i > 0) Line()
          Item(
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
            trailingContent = { Ico("chevron", null) },
          )
        }
      }
    }
  }

  when (sheet) {
    AddSheet.Choose -> AddChooser(onClose = { sheet = null }) { sheet = it }
    AddSheet.App -> AppCode(onClose = { sheet = null; shell.tick++ })
    AddSheet.ShowCode -> AddPersonFlow(shell, showCode = true, onClose = { sheet = null })
    AddSheet.EnterCode -> AddPersonFlow(shell, showCode = false, onClose = { sheet = null })
    AddSheet.Ai -> Info(
      "Connect an AI on a computer",
      "On the computer, open UUOB and choose Access → Add → An AI.",
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
      Ico("ai", null, tint = Color.White)
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
  Surface(color = color.copy(alpha = 0.16f), contentColor = color, shape = RoundedCornerShape(6.dp), modifier = Modifier.padding(start = 8.dp)) {
    Text(text.uppercase(), Modifier.padding(horizontal = 7.dp, vertical = 2.dp), style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Bold)
  }
}

/** "What are you adding?": an app, a person or an AI. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddChooser(onClose: () -> Unit, onChoose: (AddSheet) -> Unit) {
  app.uuob.kit.DetailSheet("Add", onClose) {
    Column(Modifier.padding(top = 10.dp)) {
      Kind("apps", "An app, with a code") { onChoose(AddSheet.App) }
      Kind("person", "A person: show my code") { onChoose(AddSheet.ShowCode) }
      Kind("link", "A person: enter their code") { onChoose(AddSheet.EnterCode) }
      Kind("ai", "An AI", last = true) { onChoose(AddSheet.Ai) }
    }
  }
}

/** A choice in Add: an icon row with a chevron. */
@Composable
private fun Kind(icon: String, title: String, last: Boolean = false, onClick: () -> Unit) =
  app.uuob.kit.IconRow(icon, Modifier.clickable(onClick = onClick), last = last) {
    Text(title, Modifier.weight(1f), fontSize = 15.sp)
    Ico("chevron", null, size = 16.dp, tint = Look.text3)
  }

/** A title, a paragraph and Done. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Info(title: String, text: String, onClose: () -> Unit) {
  app.uuob.kit.DetailSheet(title, onClose) {
    Caption(text, Modifier.padding(top = 6.dp))
  }
}
