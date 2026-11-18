package app.uuob

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

/** A group of rows on one card, with an optional label above and a note below. */
@Composable
fun Section(label: String? = null, note: String? = null, content: @Composable ColumnScope.() -> Unit) {
  Column(Modifier.padding(top = 20.dp)) {
    if (label != null) Text(label, style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(start = 16.dp, bottom = 8.dp))
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer), shape = RoundedCornerShape(24.dp)) {
      Column(Modifier.padding(vertical = 4.dp), content = content)
    }
    if (note != null) Caption(note, Modifier.padding(start = 16.dp, end = 16.dp, top = 8.dp))
  }
}

@Composable
fun Caption(text: String, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onSurfaceVariant) =
  Text(text, modifier = modifier, style = MaterialTheme.typography.bodyMedium, color = color)

/** The intro shown on a screen that has nothing in it yet. */
@Composable
fun Empty(icon: androidx.compose.ui.graphics.vector.ImageVector, title: String, text: String) {
  Card(Modifier.fillMaxWidth().padding(top = 20.dp), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer), shape = RoundedCornerShape(24.dp)) {
    Column(Modifier.fillMaxWidth().padding(28.dp), horizontalAlignment = Alignment.CenterHorizontally) {
      Box(Modifier.size(60.dp).clip(RoundedCornerShape(18.dp)).background(MaterialTheme.colorScheme.secondaryContainer), contentAlignment = Alignment.Center) {
        Icon(icon, null, Modifier.size(30.dp), tint = MaterialTheme.colorScheme.onSecondaryContainer)
      }
      Text(title, style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 12.dp))
      Caption(text, Modifier.padding(top = 4.dp), )
    }
  }
}

/** Online or offline. */
@Composable
fun Dot(on: Boolean) = Box(Modifier.size(8.dp).clip(CircleShape).background(if (on) Tint.ok else Tint.offline))

/** Every app gets a tile the same way: a colour from its id and its first letter. UUOB knows no app in particular. */
fun appColor(app: String) = Tint.apps[Math.floorMod(app.hashCode(), Tint.apps.size)]

@Composable
fun AppTile(app: String, size: Dp = 44.dp) =
  Box(Modifier.size(size).clip(RoundedCornerShape(size * 0.28f)).background(appColor(app)), contentAlignment = Alignment.Center) {
    Text(initial(app), color = Color.White, fontWeight = FontWeight.SemiBold, fontSize = (size.value * 0.46f).sp)
  }

fun initial(name: String?) = name?.trim()?.firstOrNull()?.uppercase() ?: "?"

@Composable
fun Avatar(name: String?, size: Dp = 44.dp, mine: Boolean = false) =
  Box(Modifier.size(size).clip(CircleShape).background(if (mine) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.tertiary), contentAlignment = Alignment.Center) {
    Text(initial(name), color = if (mine) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onTertiary, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
  }

/** A choice between a few values, e.g. Can view / Can edit. */
@Composable
fun Choice(options: List<Pair<String, String>>, selected: String, modifier: Modifier = Modifier, onSelect: (String) -> Unit) {
  SingleChoiceSegmentedButtonRow(modifier) {
    options.forEachIndexed { i, (value, label) ->
      // No tick: the filled segment already says which one is chosen, and the label needs the room.
      SegmentedButton(selected = value == selected, onClick = { onSelect(value) }, shape = SegmentedButtonDefaults.itemShape(i, options.size), icon = {}) {
        Text(label, maxLines = 2, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
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
  OutlinedTextField(
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
  AlertDialog(
    onDismissRequest = onDismiss,
    title = { Text(title) },
    text = { Text(text) },
    confirmButton = {
      Button(onClick = onConfirm, colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error, contentColor = MaterialTheme.colorScheme.onError)) { Text(action) }
    },
    dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
  )

/** The content of a bottom sheet: padded, with room for the keyboard. */
@Composable
fun SheetBody(content: @Composable ColumnScope.() -> Unit) =
  Column(Modifier.fillMaxWidth().widthIn(max = 560.dp).padding(PaddingValues(start = 24.dp, end = 24.dp, bottom = 32.dp)), content = content)

@Composable
fun SheetTitle(text: String, center: Boolean = false) =
  Text(text, Modifier.fillMaxWidth(), style = MaterialTheme.typography.headlineSmall, textAlign = if (center) TextAlign.Center else TextAlign.Start)

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
