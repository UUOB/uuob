package app.uuob

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/** Colours that mean the same everywhere: online, waiting, and the apps' tiles. */
object Tint {
  val ok = Color(0xFF30D158)
  val offline = Color(0xFF8E8E93)
  val waiting = Color(0xFFFF9F0A)
  val apps = listOf(0xFF0A84FF, 0xFFFF453A, 0xFFFF9F0A, 0xFF30D158, 0xFFBF5AF2, 0xFFFF375F, 0xFF40C8E0, 0xFF5E5CE6).map { Color(it) }
  /** A person's face: the same green to blue as UUOB always had. */
  val person = Brush.linearGradient(listOf(Color(0xFF30C48D), Color(0xFF2A9DF4)))
}

// UUOB's own look, the one its screens had as a web page (ui/tokens.css):
// near black or soft grey, cards with a hairline edge, one blue. Not the
// phone's wallpaper colours: UUOB looks the same on every phone.
private val darkScheme = darkColorScheme(
  primary = Color(0xFF0A84FF), onPrimary = Color.White,
  primaryContainer = Color(0xFF0E2A47), onPrimaryContainer = Color(0xFF7DBBFF),
  secondary = Color(0xFF0A84FF), onSecondary = Color.White,
  secondaryContainer = Color(0xFF26262A), onSecondaryContainer = Color(0xFFF5F5F7),
  tertiary = Color(0xFF40C8E0), onTertiary = Color(0xFF0B0B0D),
  background = Color(0xFF0B0B0D), onBackground = Color(0xFFF5F5F7),
  surface = Color(0xFF0B0B0D), onSurface = Color(0xFFF5F5F7),
  surfaceVariant = Color(0xFF26262A), onSurfaceVariant = Color(0xFF9D9DA1),
  surfaceContainerLowest = Color(0xFF0B0B0D), surfaceContainerLow = Color(0xFF161618),
  surfaceContainer = Color(0xFF1C1C1F), surfaceContainerHigh = Color(0xFF26262A), surfaceContainerHighest = Color(0xFF2E2E33),
  outline = Color(0xFF3A3A3F), outlineVariant = Color(0xFF26262A),
  error = Color(0xFFFF453A), onError = Color.White, errorContainer = Color(0xFF3A1513), onErrorContainer = Color(0xFFFF8A80),
)

private val lightScheme = lightColorScheme(
  primary = Color(0xFF007AFF), onPrimary = Color.White,
  primaryContainer = Color(0xFFE0EEFF), onPrimaryContainer = Color(0xFF0058B8),
  secondary = Color(0xFF007AFF), onSecondary = Color.White,
  secondaryContainer = Color(0xFFE9E9EE), onSecondaryContainer = Color(0xFF1D1D1F),
  tertiary = Color(0xFF30B0C7), onTertiary = Color.White,
  background = Color(0xFFF5F5F7), onBackground = Color(0xFF1D1D1F),
  surface = Color(0xFFF5F5F7), onSurface = Color(0xFF1D1D1F),
  surfaceVariant = Color(0xFFF0F0F3), onSurfaceVariant = Color(0xFF6E6E73),
  surfaceContainerLowest = Color.White, surfaceContainerLow = Color(0xFFFAFAFB),
  surfaceContainer = Color.White, surfaceContainerHigh = Color(0xFFF0F0F3), surfaceContainerHighest = Color(0xFFE8E8ED),
  outline = Color(0xFFC7C7CC), outlineVariant = Color(0xFFE5E5EA),
  error = Color(0xFFFF3B30), onError = Color.White, errorContainer = Color(0xFFFFE5E3), onErrorContainer = Color(0xFFB3261E),
)

/** Apple-like type: a large bold title, 17 point text, tight letters. */
private val type = Typography().let { t ->
  val tight = (-0.01).em
  t.copy(
    headlineMedium = TextStyle(fontSize = 34.sp, lineHeight = 40.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.025).em),
    titleLarge = TextStyle(fontSize = 20.sp, lineHeight = 26.sp, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.02).em),
    titleMedium = t.titleMedium.copy(fontSize = 17.sp, fontWeight = FontWeight.SemiBold, letterSpacing = tight),
    titleSmall = t.titleSmall.copy(fontSize = 15.sp, fontWeight = FontWeight.SemiBold, letterSpacing = tight),
    bodyLarge = t.bodyLarge.copy(fontSize = 17.sp, lineHeight = 24.sp, letterSpacing = tight),
    bodyMedium = t.bodyMedium.copy(fontSize = 15.sp, lineHeight = 21.sp, letterSpacing = tight),
    bodySmall = t.bodySmall.copy(fontSize = 13.sp, lineHeight = 18.sp, letterSpacing = 0.em),
    labelLarge = t.labelLarge.copy(fontSize = 15.sp, fontWeight = FontWeight.SemiBold, letterSpacing = tight),
    labelMedium = t.labelMedium.copy(fontSize = 13.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.em),
    labelSmall = t.labelSmall.copy(fontSize = 11.sp, fontWeight = FontWeight.Medium, letterSpacing = 0.em),
  )
}

/** UUOB's colours, light or dark as the phone is. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun UuobTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) =
  MaterialExpressiveTheme(colorScheme = if (dark) darkScheme else lightScheme, typography = type, motionScheme = MotionScheme.expressive(), content = content)
