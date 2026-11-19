package app.uuob

import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.selectable
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.luminance
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import kotlinx.coroutines.delay
import org.json.JSONObject

/** List items on a card: no background of their own. */
@Composable
fun clearItem() = ListItemDefaults.colors(containerColor = Color.Transparent)

/** A group of rows on one card (a hairline edge, round corners), with an
 *  optional small uppercase label above and a note below. */
@Composable
fun Section(label: String? = null, note: String? = null, content: @Composable ColumnScope.() -> Unit) {
  Column(Modifier.padding(top = if (label != null) 28.dp else 20.dp)) {
    if (label != null) SectionLabel(label, Modifier.padding(start = 4.dp, bottom = 8.dp))
    Card(
      colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
      shape = RoundedCornerShape(18.dp),
      border = cardEdge,
    ) {
      Column(Modifier.fillMaxWidth(), content = content)
    }
    if (note != null) Text(note, Modifier.padding(start = 4.dp, end = 4.dp, top = 10.dp), style = MaterialTheme.typography.bodySmall, color = Look.text2)
  }
}

/** A section's label: small, grey, in capitals. */
@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier) =
  Text(text.uppercase(), modifier, style = androidx.compose.ui.text.TextStyle(fontSize = 13.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.52.sp), color = Look.text3)


@Composable
fun Caption(text: String, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onSurfaceVariant) =
  Text(text, modifier = modifier, style = MaterialTheme.typography.bodyMedium, color = color)

/** The intro shown on a screen that has nothing in it yet. */
@Composable
fun Empty(icon: String, title: String, text: String) {
  // `.empty`: centred, a large line icon, the title and a grey line.
  Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 48.dp), horizontalAlignment = Alignment.CenterHorizontally) {
    Ico(icon, null, size = 44.dp, tint = Look.text3, stroke = 1.5f)
    Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 12.dp))
    Text(text, Modifier.padding(top = 6.dp), style = MaterialTheme.typography.bodyMedium, color = Look.text2, textAlign = TextAlign.Center)
  }
}

/** Online or offline. */
@Composable
fun Dot(on: Boolean) = Box(Modifier.size(9.dp).clip(CircleShape).background(if (on) Tint.ok else Look.text3))

/** Every app gets a tile the same way: a colour from its id and its first letter. UUOB knows no app in particular. */
/** An app's colour, the same on every platform: FNV-1a of its id (crates/uuob-app/src/screens.rs `tint`). */
fun appColor(app: String): Color {
  var h = 0x811c9dc5L
  for (b in app.toByteArray()) h = ((h xor (b.toLong() and 0xff)) * 0x01000193L) and 0xffffffffL
  return Tint.apps[(h % Tint.apps.size).toInt()]
}

@Composable
fun AppTile(app: String, size: Dp = 44.dp) =
  Box(Modifier.size(size).clip(RoundedCornerShape(size * 0.28f)).background(appColor(app)), contentAlignment = Alignment.Center) {
    Text(initial(app), color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = (size.value * 0.46f).sp)
  }

fun initial(name: String?) = name?.trim()?.firstOrNull()?.uppercase() ?: "?"

/** A person: their initial on the green to blue (yours on the blue). */
@Composable
fun Avatar(name: String?, size: Dp = 44.dp, mine: Boolean = false) =
  Box(
    Modifier.size(size).clip(CircleShape).then(if (mine) Modifier.background(MaterialTheme.colorScheme.primary) else Modifier.background(Tint.person)),
    contentAlignment = Alignment.Center,
  ) {
    Text(initial(name), color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = (size.value * 0.42f).sp)
  }

/** A choice between a few values, e.g. Can view / Can edit: a grey track
 *  with the chosen one raised on it. */
@Composable
fun Choice(options: List<Pair<String, String>>, selected: String, modifier: Modifier = Modifier, onSelect: (String) -> Unit) {
  val scheme = MaterialTheme.colorScheme
  val raised = Look.elevated
  // Stretched across when the caller gives it a width, else as wide as its labels.
  val stretch = modifier != Modifier
  Row(
    modifier.clip(RoundedCornerShape(10.dp)).background(scheme.surfaceContainerHigh).padding(3.dp).height(IntrinsicSize.Min),
    horizontalArrangement = Arrangement.spacedBy(2.dp),
  ) {
    options.forEach { (value, label) ->
      val on = value == selected
      Box(
        Modifier.then(if (stretch) Modifier.weight(1f) else Modifier).widthIn(min = 44.dp).fillMaxHeight()
          .then(if (on) Modifier.shadow(1.dp, RoundedCornerShape(8.dp)) else Modifier)
          .clip(RoundedCornerShape(8.dp)).background(if (on) raised else Color.Transparent)
          .selectable(selected = on, onClick = { onSelect(value) }, role = androidx.compose.ui.semantics.Role.Tab)
          .padding(horizontal = 14.dp, vertical = 5.dp),
        contentAlignment = Alignment.Center,
      ) {
        Text(
          label, maxLines = 1, softWrap = false, textAlign = androidx.compose.ui.text.style.TextAlign.Center,
          style = androidx.compose.ui.text.TextStyle(fontSize = 14.sp, fontWeight = FontWeight.Medium), color = if (on) scheme.onSurface else Look.text2,
        )
      }
    }
  }
}

/** A QR code of `text`, black on white so any camera reads it. */
@Composable
fun QrCode(text: String, size: Dp = 180.dp) {
  val matrix = remember(text) {
    QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, 0, 0, mapOf(EncodeHintType.MARGIN to 0, EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M))
  }
  Surface(color = Color.White, shape = RoundedCornerShape(20.dp)) {
    Canvas(Modifier.padding(16.dp).size(size)) {
      val cell = this.size.width / matrix.width
      for (y in 0 until matrix.height) for (x in 0 until matrix.width) {
        if (matrix[x, y]) drawRect(Color.Black, Offset(x * cell, y * cell), Size(cell + 0.5f, cell + 0.5f))
      }
    }
  }
}

/** An invite code with its QR code and a countdown. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CodeView(invite: JSONObject, waiting: String, qr: Boolean = true) {
  val code = invite.optString("code")
  val expires = invite.optLong("expires_at")
  var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
  LaunchedEffect(invite) {
    while (true) {
      now = System.currentTimeMillis()
      delay(1000)
    }
  }
  val left = ((expires - now) / 1000).coerceAtLeast(0)
  Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(8.dp)) {
      code.split("-").forEachIndexed { i, part ->
        Surface(
          color = if (i == 0) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.surfaceContainerHigh,
          contentColor = if (i == 0) MaterialTheme.colorScheme.onPrimaryContainer else MaterialTheme.colorScheme.onSurface,
          shape = RoundedCornerShape(14.dp),
        ) {
          Text(part, Modifier.padding(horizontal = 14.dp, vertical = 10.dp), style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.SemiBold)
        }
      }
    }
    if (qr) {
      Spacer(Modifier.height(20.dp))
      QrCode(code, 160.dp)
    }
    Row(Modifier.padding(top = 18.dp), verticalAlignment = Alignment.CenterVertically) {
      CircularProgressIndicator(Modifier.size(14.dp), strokeWidth = 2.dp)
      Caption("  $waiting Code expires in ${left / 60}:${"%02d".format(left % 60)}")
    }
  }
}

/** A field for a code like "42 amber river falcon". */
@Composable
fun CodeField(value: String, onChange: (String) -> Unit, onGo: () -> Unit) =
  Field(
    value, onChange, Modifier.fillMaxWidth(),
    placeholder = { Text("e.g. 42 amber river falcon") },
    singleLine = true,
    textStyle = MaterialTheme.typography.titleLarge.copy(textAlign = TextAlign.Center),
    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, keyboardType = KeyboardType.Password, imeAction = ImeAction.Go),
    keyboardActions = KeyboardActions(onGo = { if (complete(value)) onGo() }),
  )

/** Four words or more: a code worth trying. */
fun complete(code: String) = code.trim().split(Regex("[\\s-]+")).size >= 4

/** A question before something that can't be undone. */
@Composable
fun Confirm(title: String, text: String, action: String, onConfirm: () -> Unit, onDismiss: () -> Unit) =
  Sheet(onDismissRequest = onDismiss) {
    SheetBody {
      SheetTitle(title)
      Secondary(text, Modifier.padding(top = 8.dp))
      Actions {
        UTonal(onClick = onDismiss, Modifier.weight(1f), large = true) { Text("Cancel") }
        UDanger(onClick = onConfirm, Modifier.weight(1f), large = true) { Text(action) }
      }
    }
  }

/** The content of a sheet: 28 points of room on every side; it scrolls if long. */
@Composable
fun SheetBody(content: @Composable ColumnScope.() -> Unit) =
  Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(28.dp), content = content)

@Composable
fun SheetTitle(text: String, center: Boolean = false) =
  Text(text, Modifier.fillMaxWidth(), style = androidx.compose.ui.text.TextStyle(fontSize = 22.sp, lineHeight = 28.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.44).sp), textAlign = if (center) TextAlign.Center else TextAlign.Start)

fun copy(ctx: Context, text: String) {
  ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("UUOB", text))
}

fun size(b: Long) = when {
  b < 1024 -> "$b B"
  b < 1 shl 20 -> "%.1f KB".format(b / 1024.0)
  b < 1 shl 30 -> "%.1f MB".format(b / 1048576.0)
  else -> "%.1f GB".format(b / 1073741824.0)
}

fun plural(n: Int, one: String, many: String) = "$n ${if (n == 1) one else many}"

fun day(ms: Long): String = java.text.DateFormat.getDateInstance(java.text.DateFormat.MEDIUM).format(java.util.Date(ms))
