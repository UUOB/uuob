package app.uuob

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ProvideTextStyle
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

// UUOB's own controls, the same as when its screens were a web page
// (ui/tokens.css): pill buttons, grouped rows, the green switch, centred
// sheets and its line icons. Sizes and colours are taken from there.

/** The extra colours the tokens have and Material doesn't. */
object Look {
  val dark @Composable get() = MaterialTheme.colorScheme.background.luminance() < 0.5f
  /** `--bg-elevated`: sheets, the chosen segment, toasts. */
  val elevated @Composable get() = if (dark) Color(0xFF161618) else Color.White
  /** `--line` and `--line-strong`. */
  val line @Composable get() = if (dark) Color.White.copy(alpha = 0.08f) else Color.Black.copy(alpha = 0.08f)
  val lineStrong @Composable get() = if (dark) Color.White.copy(alpha = 0.14f) else Color.Black.copy(alpha = 0.14f)
  /** `--text-2`, `--text-3`. */
  val text2 @Composable get() = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.62f)
  val text3 @Composable get() = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f)
  /** `--hover`, `--active`: the tab bar's tiles. */
  val active @Composable get() = if (dark) Color.White.copy(alpha = 0.1f) else Color.Black.copy(alpha = 0.08f)
  val accentSoft @Composable get() = MaterialTheme.colorScheme.primary.copy(alpha = if (dark) 0.16f else 0.12f)
  val dangerSoft @Composable get() = MaterialTheme.colorScheme.error.copy(alpha = if (dark) 0.14f else 0.1f)
}

// ---------- icons ----------

/** UUOB's line icons (24 point grid, drawn with a 1.75 stroke), as ui/Icon.svelte had them. */
private val paths = mapOf(
  "plus" to "M12 5v14M5 12h14",
  "trash" to "M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3",
  "link" to "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
  "close" to "M6 6l12 12M18 6L6 18",
  "check" to "M5 12l4 4 10-10",
  "copy" to "M9 9h10v10a1 1 0 0 1-1 1H10a1 1 0 0 1-1-1zM5 15V5a1 1 0 0 1 1-1h9",
  "lock" to "M6 11h12v9H6zM8 11V8a4 4 0 0 1 8 0v3",
  "chevron" to "M9 6l6 6-6 6",
  "back" to "M15 6l-6 6 6 6",
  "right" to "M5 12h14M13 6l6 6-6 6",
  "edit" to "M4 20h4L19 9l-4-4L4 16zM13 7l4 4",
  "laptop" to "M4 6a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v9H4zM2 18h20",
  "phone" to "M8 3h8a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM11 18h2",
  "server" to "M5 4h14a1 1 0 0 1 1 1v5H4V5a1 1 0 0 1 1-1zM4 14h16v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1zM8 7h.01M8 17h.01",
  "search" to "M11 4a7 7 0 1 1 0 14 7 7 0 0 1 0-14zM20 20l-4-4",
  "scan" to "M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3M7 12h10",
  "question" to "M9.1 9a3 3 0 0 1 5.8 1c0 2-3 2.5-3 4.5M12 18h.01",
  "settings" to "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
  "people" to "M9 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zM2.5 20a6.5 6.5 0 0 1 13 0M16 4.2a3.5 3.5 0 0 1 0 6.6M18 14.3a6.5 6.5 0 0 1 3.5 5.7",
  "person" to "M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4.5 21a7.5 7.5 0 0 1 15 0",
  "ai" to "M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8zM19 15l.8 2.2 2.2.8-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z",
  "apps" to "M5 4h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM15 4h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zM5 14h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1zM15 14h4a1 1 0 0 1 1 1v4a1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1v-4a1 1 0 0 1 1-1z",
  "network" to "M14.5 5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM7.5 18a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM21.5 18a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM10.8 7.2L6.2 15.8M13.2 7.2l4.6 8.6M7.5 18h9",
  "shield" to "M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6zM9 12l2 2 4-4",
)

private val vectors = HashMap<String, ImageVector>()

private fun vector(name: String, stroke: Float): ImageVector = vectors.getOrPut("$name/$stroke") {
  ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f).apply {
    addPath(
      PathParser().parsePathString(paths[name] ?: "").toNodes(),
      stroke = SolidColor(Color.Black), strokeLineWidth = stroke, strokeLineCap = StrokeCap.Round, strokeLineJoin = StrokeJoin.Round,
    )
  }.build()
}

/** One of UUOB's line icons, `name` as in [paths]. */
@Composable
fun Ico(name: String, contentDescription: String? = null, modifier: Modifier = Modifier, tint: Color = LocalContentColor.current, size: Dp = 22.dp, stroke: Float = 1.75f) =
  androidx.compose.material3.Icon(rememberVectorPainter(vector(name, stroke)), contentDescription, modifier.size(size), tint = tint)

// ---------- buttons ----------

enum class Kind { Plain, Primary, Danger, Ghost }

/** `.btn`: a pill, 40 points high (`.lg`: 50), grey, blue, red or just blue text. */
@Composable
fun Btn(onClick: () -> Unit, modifier: Modifier = Modifier, kind: Kind = Kind.Plain, large: Boolean = false, enabled: Boolean = true, content: @Composable RowScope.() -> Unit) {
  val scheme = MaterialTheme.colorScheme
  val (bg, fg) = when (kind) {
    Kind.Plain -> scheme.surfaceContainerHigh to scheme.onSurface
    Kind.Primary -> scheme.primary to Color.White
    Kind.Danger -> Look.dangerSoft to scheme.error
    Kind.Ghost -> Color.Transparent to scheme.primary
  }
  val press = remember { MutableInteractionSource() }
  val pressed by press.collectIsPressedAsState()
  val scale by animateFloatAsState(if (pressed) 0.97f else 1f, label = "press")
  Row(
    modifier.scale(scale).alpha(if (enabled) 1f else 0.45f).heightIn(min = if (large) 50.dp else 40.dp).clip(CircleShape).background(bg)
      .clickable(press, androidx.compose.material3.ripple(color = fg), enabled = enabled, role = Role.Button, onClick = onClick)
      .padding(horizontal = if (large) 26.dp else 18.dp),
    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    CompositionLocalProvider(LocalContentColor provides fg) {
      ProvideTextStyle(TextStyle(color = fg, fontSize = if (large) 17.sp else 15.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.15).sp)) { content() }
    }
  }
}

// The same names as Material's buttons had in these screens, so each reads as before.
@Composable fun UButton(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, large: Boolean = false, content: @Composable RowScope.() -> Unit) =
  Btn(onClick, modifier, Kind.Primary, large, enabled, content)
@Composable fun UTonal(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, large: Boolean = false, content: @Composable RowScope.() -> Unit) =
  Btn(onClick, modifier, Kind.Plain, large, enabled, content)
@Composable fun UGhost(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, large: Boolean = false, content: @Composable RowScope.() -> Unit) =
  Btn(onClick, modifier, Kind.Ghost, large, enabled, content)
@Composable fun UDanger(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, large: Boolean = false, content: @Composable RowScope.() -> Unit) =
  Btn(onClick, modifier, Kind.Danger, large, enabled, content)

/** `.icon-btn`: a round 36 point button, grey until touched. */
@Composable
fun IconBtn(onClick: () -> Unit, modifier: Modifier = Modifier, content: @Composable () -> Unit) =
  Box(
    modifier.size(36.dp).clip(CircleShape).clickable(role = Role.Button, onClick = onClick),
    contentAlignment = Alignment.Center,
  ) { CompositionLocalProvider(LocalContentColor provides Look.text2) { content() } }

// ---------- rows ----------

/**
 * `.row` in a `.group`: 14 by 18 points of padding, at least 56 high, a
 * semibold line with a grey caption under it. (The parameters are Material's
 * ListItem's, so the screens read as before; `colors` is not used.)
 */
@Composable
fun Item(
  headlineContent: @Composable () -> Unit,
  modifier: Modifier = Modifier,
  supportingContent: (@Composable () -> Unit)? = null,
  leadingContent: (@Composable () -> Unit)? = null,
  trailingContent: (@Composable () -> Unit)? = null,
  @Suppress("UNUSED_PARAMETER") colors: Any? = null,
) = Row(
  modifier.fillMaxWidth().heightIn(min = 56.dp).padding(horizontal = 18.dp, vertical = 14.dp),
  horizontalArrangement = Arrangement.spacedBy(14.dp), verticalAlignment = Alignment.CenterVertically,
) {
  leadingContent?.invoke()
  Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
    ProvideTextStyle(MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold)) { headlineContent() }
    if (supportingContent != null) {
      CompositionLocalProvider(LocalContentColor provides Look.text2) {
        ProvideTextStyle(MaterialTheme.typography.bodySmall.copy(color = Look.text2)) { supportingContent() }
      }
    }
  }
  if (trailingContent != null) CompositionLocalProvider(LocalContentColor provides Look.text2) { trailingContent() }
}

/** The hairline between two rows. */
@Composable
fun Line(modifier: Modifier = Modifier) = Box(modifier.fillMaxWidth().height(1.dp).background(Look.line))

// ---------- switch ----------

/** `.switch`: 51 by 31, green when on; the knob springs across. */
@Composable
fun Toggle(checked: Boolean, onChange: (Boolean) -> Unit) {
  val x by animateDpAsState(if (checked) 20.dp else 0.dp, spring(dampingRatio = 0.6f, stiffness = 500f), label = "knob")
  Box(
    Modifier.size(51.dp, 31.dp).clip(CircleShape).background(if (checked) Tint.ok else Look.lineStrong)
      .clickable(role = Role.Switch) { onChange(!checked) }
      .semantics { contentDescription = if (checked) "On" else "Off" },
  ) {
    Box(Modifier.padding(2.dp).offset(x = x).size(27.dp).shadow(3.dp, CircleShape).clip(CircleShape).background(Color.White))
  }
}

// ---------- text field ----------

/** `.field`: 46 high, rounded, on grey; a blue ring while typing. */
@Composable
fun Field(
  value: String,
  onValueChange: (String) -> Unit,
  modifier: Modifier = Modifier,
  placeholder: (@Composable () -> Unit)? = null,
  @Suppress("UNUSED_PARAMETER") singleLine: Boolean = true,
  textStyle: TextStyle? = null,
  keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
  keyboardActions: KeyboardActions = KeyboardActions.Default,
) {
  val scheme = MaterialTheme.colorScheme
  val focus = remember { MutableInteractionSource() }
  val focused by focus.collectIsFocusedAsState()
  val style = (textStyle ?: MaterialTheme.typography.bodyLarge).copy(color = scheme.onSurface)
  BasicTextField(
    value, onValueChange,
    modifier.fillMaxWidth()
      .then(if (focused) Modifier.border(4.dp, Look.accentSoft, RoundedCornerShape(15.dp)).padding(3.dp) else Modifier.padding(3.dp))
      .heightIn(min = 46.dp).clip(RoundedCornerShape(12.dp))
      .background(if (focused) scheme.surfaceContainer else scheme.surfaceContainerHigh)
      .border(1.dp, if (focused) scheme.primary else Look.line, RoundedCornerShape(12.dp)),
    textStyle = style, singleLine = true, keyboardOptions = keyboardOptions, keyboardActions = keyboardActions,
    interactionSource = focus, cursorBrush = SolidColor(scheme.primary),
    decorationBox = { inner ->
      Box(Modifier.padding(horizontal = 14.dp, vertical = 12.dp), contentAlignment = if (style.textAlign == androidx.compose.ui.text.style.TextAlign.Center) Alignment.Center else Alignment.CenterStart) {
        if (value.isEmpty() && placeholder != null) {
          CompositionLocalProvider(LocalContentColor provides Look.text3) { ProvideTextStyle(style.copy(color = Look.text3)) { placeholder() } }
        }
        inner()
      }
    },
  )
}

// ---------- sheets ----------

/**
 * `.sheet`: a card in the middle of the screen over a dimmed one, 26 point
 * corners. Tapping outside or back closes it (`onDismissRequest`). (The other
 * parameters are the ones Material's bottom sheet took here; they are not used.)
 */
@Composable
fun Sheet(
  onDismissRequest: () -> Unit,
  @Suppress("UNUSED_PARAMETER") sheetState: Any? = null,
  @Suppress("UNUSED_PARAMETER") dragHandle: Any? = null,
  content: @Composable ColumnScope.() -> Unit,
) = Dialog(onDismissRequest, DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
  Box(
    Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.45f))
      .clickable(remember { MutableInteractionSource() }, null, onClick = onDismissRequest)
      .padding(16.dp),
    contentAlignment = Alignment.Center,
  ) {
    Column(
      Modifier.widthIn(max = 460.dp).fillMaxWidth().shadow(24.dp, RoundedCornerShape(26.dp))
        .clip(RoundedCornerShape(26.dp)).background(Look.elevated).border(1.dp, Look.line, RoundedCornerShape(26.dp))
        // Taps on the card stay on the card.
        .clickable(remember { MutableInteractionSource() }, null) {},
    ) {
      // A dialog starts with no colours of its own: the page's text colour and type.
      CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) {
        ProvideTextStyle(MaterialTheme.typography.bodyLarge) { content() }
      }
    }
  }
}

/** Two buttons side by side at the bottom of a sheet (`.actions`). */
@Composable
fun Actions(content: @Composable RowScope.() -> Unit) =
  Row(Modifier.fillMaxWidth().padding(top = 22.dp), horizontalArrangement = Arrangement.spacedBy(10.dp), content = content)

/** A grey lead line, as under a page's or a sheet's title. */
@Composable
fun Secondary(text: String, modifier: Modifier = Modifier, center: Boolean = false) =
  Text(text, modifier, style = MaterialTheme.typography.bodyLarge, color = Look.text2, textAlign = if (center) androidx.compose.ui.text.style.TextAlign.Center else null)

/** `.tag`: "This device", small and blue in a pill. */
@Composable
fun Tag(text: String) =
  Text(
    text, Modifier.padding(start = 6.dp).clip(CircleShape).background(Look.accentSoft).padding(horizontal = 8.dp, vertical = 2.dp),
    style = TextStyle(fontSize = 12.sp, fontWeight = FontWeight.SemiBold), color = MaterialTheme.colorScheme.primary,
  )

/** A border for cards: `--line`. */
val cardEdge @Composable get() = BorderStroke(1.dp, Look.line)
