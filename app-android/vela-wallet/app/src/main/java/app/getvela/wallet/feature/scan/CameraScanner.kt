package app.getvela.wallet.feature.scan

import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import app.getvela.wallet.core.diagnostics.VelaLog
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/**
 * The camera behind the scanner's brackets (spec 046 D4): a CameraX preview
 * and one analysis stream on a single executor, every frame handed to
 * [QrDecoder]; the first hit is reported once and the analysis stops.
 * `onUnavailable` fires when the camera cannot be opened (another app has it,
 * no camera at all) — the photo path still works then.
 */
@Composable
fun CameraScanner(
    torch: Boolean,
    onDecoded: (String) -> Unit,
    onUnavailable: () -> Unit,
    modifier: Modifier = Modifier,
    /** Spec 048: which camera — 翻转 rebinds to the other one when the device has it. */
    lensFacing: Int = CameraSelector.LENS_FACING_BACK,
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val decoded = rememberUpdatedState(onDecoded)
    val unavailable = rememberUpdatedState(onUnavailable)
    val previewView = remember { PreviewView(context).apply { implementationMode = PreviewView.ImplementationMode.COMPATIBLE } }
    val executor = remember { Executors.newSingleThreadExecutor() }
    val reported = remember { AtomicBoolean(false) }
    var camera by remember { mutableStateOf<Camera?>(null) }
    var provider by remember { mutableStateOf<ProcessCameraProvider?>(null) }

    LaunchedEffect(lensFacing) {
        val future = ProcessCameraProvider.getInstance(context)
        future.addListener({
            val cameraProvider = runCatching { future.get() }.getOrNull()
            if (cameraProvider == null) {
                unavailable.value(); return@addListener
            }
            val preview = Preview.Builder().build().also { it.surfaceProvider = previewView.surfaceProvider }
            val analysis = ImageAnalysis.Builder()
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .build()
            analysis.setAnalyzer(executor) { image ->
                try {
                    if (!reported.get()) {
                        val plane = image.planes[0]
                        val buffer = plane.buffer
                        val bytes = ByteArray(buffer.remaining()).also { buffer.get(it) }
                        val text = QrDecoder.decodeYuv(bytes, image.width, image.height, plane.rowStride)
                        if (text != null && reported.compareAndSet(false, true)) {
                            ContextCompat.getMainExecutor(context).execute { decoded.value(text) }
                        }
                    }
                } finally {
                    image.close()
                }
            }
            try {
                cameraProvider.unbindAll()
                val wanted = CameraSelector.Builder().requireLensFacing(lensFacing).build()
                val selector = if (runCatching { cameraProvider.hasCamera(wanted) }.getOrDefault(false)) wanted else CameraSelector.DEFAULT_BACK_CAMERA
                camera = cameraProvider.bindToLifecycle(lifecycleOwner, selector, preview, analysis)
                provider = cameraProvider
            } catch (error: Exception) {
                VelaLog.failure("scan.camera", "bind failed", error)
                unavailable.value()
            }
        }, ContextCompat.getMainExecutor(context))
    }
    LaunchedEffect(torch, camera) { camera?.cameraControl?.enableTorch(torch) }
    DisposableEffect(Unit) {
        onDispose {
            runCatching { provider?.unbindAll() }
            executor.shutdown()
        }
    }
    AndroidView(factory = { previewView }, modifier = modifier)
}
