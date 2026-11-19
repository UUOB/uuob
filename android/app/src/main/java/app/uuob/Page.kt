package app.uuob

import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.statusBarsPadding
import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.zxing.BinaryBitmap
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import java.util.concurrent.Executors

/**
 * A screen, as UUOB's pages always were: a large bold title with a grey line
 * about it, then its content, all scrolling together. With `onBack`, a back
 * button above the title (and the back gesture) leaves it.
 */
@Composable
fun Page(title: String, lead: String, top: @Composable () -> Unit, onBack: (() -> Unit)? = null, content: LazyListScope.() -> Unit) {
  if (onBack != null) androidx.activity.compose.BackHandler(onBack = onBack)
  LazyColumn(
    Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).statusBarsPadding(),
    contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = if (onBack != null) 8.dp else 28.dp, bottom = 32.dp),
  ) {
    block {
      if (onBack != null) {
        IconBtn(onClick = onBack, Modifier.padding(bottom = 8.dp).offset(x = (-8).dp)) { Ico("back", "Back", size = 24.dp) }
      }
      androidx.compose.foundation.layout.Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
          title, Modifier.weight(1f),
          style = androidx.compose.ui.text.TextStyle(fontSize = 30.sp, lineHeight = 36.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.Bold, letterSpacing = (-0.025).em),
          color = MaterialTheme.colorScheme.onSurface,
        )
        top()
      }
      if (lead.isNotEmpty()) Secondary(lead, Modifier.padding(top = 10.dp))
      Spacer(Modifier.height(4.dp))
    }
    content()
  }
}

/** One part of a page, no wider than reads well on a tablet. */
fun LazyListScope.block(key: Any? = null, content: @Composable ColumnScope.() -> Unit) = item(key) {
  Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.TopCenter) {
    Column(Modifier.widthIn(max = 680.dp).fillMaxWidth(), content = content)
  }
}

/**
 * The camera, full screen, until it sees a QR code: `onDone(text, null)`.
 * `onDone(null, why)` when it can't scan, `onDone(null, null)` when cancelled.
 */
@Composable
fun Scanner(onDone: (String?, String?) -> Unit) {
  val ctx = LocalContext.current
  val lifecycle = LocalLifecycleOwner.current
  var allowed by remember { mutableStateOf(ctx.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
  val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { ok ->
    if (ok) allowed = true else onDone(null, "UUOB needs the camera to scan. Allow it in the phone’s settings, or type the code.")
  }
  LaunchedEffect(Unit) { if (!allowed) ask.launch(Manifest.permission.CAMERA) }
  val provider = remember { ProcessCameraProvider.getInstance(ctx) }
  DisposableEffect(Unit) { onDispose { if (provider.isDone) provider.get().unbindAll() } }

  Dialog(onDismissRequest = { onDone(null, null) }, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
    Box(Modifier.fillMaxSize().background(Color.Black)) {
      if (allowed) AndroidView(
        factory = { c ->
          PreviewView(c).also { view ->
            provider.addListener({
              val preview = Preview.Builder().build().also { it.setSurfaceProvider(view.surfaceProvider) }
              val analysis = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
              var found = false
              analysis.setAnalyzer(Executors.newSingleThreadExecutor()) { image ->
                val text = read(image)
                image.close()
                if (text != null && !found) {
                  found = true
                  ContextCompat.getMainExecutor(c).execute { onDone(text, null) }
                }
              }
              try {
                provider.get().unbindAll()
                provider.get().bindToLifecycle(lifecycle, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
              } catch (e: Exception) {
                onDone(null, "The camera could not be opened. Type the code instead.")
              }
            }, ContextCompat.getMainExecutor(c))
          }
        },
        modifier = Modifier.fillMaxSize(),
      )
      Box(Modifier.align(Alignment.Center).padding(bottom = 96.dp).size(260.dp).border(3.dp, Color.White.copy(alpha = 0.9f), RoundedCornerShape(28.dp)))
      Column(
        Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(14.dp),
      ) {
        Text("Point the camera at the QR code", color = Color.White, style = MaterialTheme.typography.titleMedium)
        // A light button on the camera picture, whatever the phone's theme.
        UuobTheme(dark = false) {
          UTonal(onClick = { onDone(null, null) }, Modifier.widthIn(max = 320.dp).fillMaxWidth(), large = true) { Text("Cancel") }
        }
      }
    }
  }
}

/** The text of a QR code in the camera picture, if there is one. */
private fun read(image: ImageProxy): String? {
  val plane = image.planes[0]
  val data = ByteArray(plane.buffer.remaining()).also { plane.buffer.get(it) }
  val source = PlanarYUVLuminanceSource(data, plane.rowStride, image.height, 0, 0, image.width, image.height, false)
  return try {
    QRCodeReader().decode(BinaryBitmap(HybridBinarizer(source))).text
  } catch (_: Exception) {
    null
  }
}
