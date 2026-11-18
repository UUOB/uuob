package app.uuob

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialExpressiveTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

/** Colours that mean the same everywhere: online, the apps' own colours. */
object Tint {
  val ok = Color(0xFF30D158)
  val offline = Color(0xFF8E8E93)
  val tasks = Color(0xFF0A84FF)
  val calendar = Color(0xFFFF453A)
  val photos = Color(0xFFFF9F0A)
  val other = Color(0xFFBF5AF2)
}

/** Material You: the phone's own colours (Android 12+), else UUOB's. Follows dark mode. */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun UuobTheme(content: @Composable () -> Unit) {
  val dark = isSystemInDarkTheme()
  val ctx = LocalContext.current
  val colors = when {
    Build.VERSION.SDK_INT >= 31 -> if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
    dark -> darkColorScheme(
      primary = Color(0xFF0A84FF), onPrimary = Color.White,
      background = Color(0xFF0B0B0D), surface = Color(0xFF0B0B0D),
      surfaceContainer = Color(0xFF1C1C1F), surfaceContainerHigh = Color(0xFF26262A),
    )
    else -> lightColorScheme(
      primary = Color(0xFF007AFF), onPrimary = Color.White,
      background = Color(0xFFF5F5F7), surface = Color(0xFFF5F5F7),
      surfaceContainer = Color.White, surfaceContainerHigh = Color(0xFFF0F0F3),
    )
  }
  MaterialExpressiveTheme(colorScheme = colors, motionScheme = MotionScheme.expressive(), content = content)
}
