package app.uuob

import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color

/** Colours that mean the same everywhere: online, waiting, and the apps' tiles. */
object Tint {
  val ok = Color(0xFF30D158)
  val offline = Color(0xFF8E8E93)
  val waiting = Color(0xFFFF9F0A)
  val apps = listOf(0xFF0A84FF, 0xFFFF453A, 0xFFFF9F0A, 0xFF30D158, 0xFFBF5AF2, 0xFFFF375F, 0xFF40C8E0, 0xFF5E5CE6).map { Color(it) }
  /** A person's face: the same green to blue as UUOB always had. */
  val person = Brush.linearGradient(listOf(Color(0xFF30C48D), Color(0xFF2A9DF4)))
}

/** UUOB's colours and type: the kit's theme (sdk/kit/android), light or dark as the phone is. */
@Composable
fun UuobTheme(content: @Composable () -> Unit) = app.uuob.kit.AppTheme(Color.Unspecified, content)
