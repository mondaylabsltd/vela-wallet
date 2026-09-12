package app.getvela.wallet.feature.scan

import android.Manifest
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.core.platform.VelaHaptic
import androidx.camera.core.CameraSelector
import android.content.pm.PackageManager
import android.graphics.BitmapFactory
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import app.getvela.wallet.feature.flows.ScanModel
import app.getvela.wallet.feature.flows.ScanTool
import app.getvela.wallet.feature.flows.components.ScanSurface
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** What the live scanner needs from its host (spec 046 US3). */
class ScanCallbacks(
    val onDecoded: (String) -> Unit,
    val onClose: () -> Unit,
    /** The camera permission, asked through the activity's launcher. */
    val requestPermission: suspend () -> Boolean,
    /** A photo from the picker, as bytes; `null` when backed out. */
    val pickImage: suspend () -> ByteArray?,
    /** Words for the states this surface can be in. */
    val permissionText: String,
    val grantLabel: String,
    val noQrFound: String,
    val cameraUnavailable: String,
    val decodeFailed: String,
)

/**
 * The drawn scanner with a camera behind its brackets: permission first
 * (asked once on entry, re-asked from the status line), the frames decoded
 * by [QrDecoder], the photo tool through the picker, torch on its tool.
 */
@Composable
fun LiveScanSurface(model: ScanModel, callbacks: ScanCallbacks, modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var granted by remember {
        mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED)
    }
    var asked by remember { mutableStateOf(false) }
    var torch by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf<String?>(null) }
    var cameraDead by remember { mutableStateOf(false) }
    // Spec 048: 翻转 — the other camera; CameraScanner falls back to the back one when there is no front camera.
    var lensFacing by remember { mutableStateOf(CameraSelector.LENS_FACING_BACK) }
    val haptic = rememberVelaHaptic()
    LaunchedEffect(Unit) {
        if (!granted && !asked) {
            asked = true
            granted = callbacks.requestPermission()
        }
    }
    ScanSurface(
        model = model,
        modifier = modifier,
        onClose = callbacks.onClose,
        onTool = { tool ->
            when (tool) {
                ScanTool.Torch -> torch = !torch
                ScanTool.Flip -> {
                    lensFacing = if (lensFacing == CameraSelector.LENS_FACING_BACK) CameraSelector.LENS_FACING_FRONT else CameraSelector.LENS_FACING_BACK
                    haptic(VelaHaptic.Select)
                }
                ScanTool.Gallery -> scope.launch {
                    val bytes = runCatching { callbacks.pickImage() }.getOrNull() ?: return@launch
                    val text = withContext(Dispatchers.Default) {
                        runCatching { BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.let(QrDecoder::decode) }.getOrNull()
                    }
                    if (text != null) callbacks.onDecoded(text) else status = callbacks.noQrFound
                }
            }
        },
        status = when {
            !granted -> callbacks.permissionText
            cameraDead -> callbacks.cameraUnavailable
            else -> status
        },
        statusAction = if (!granted) callbacks.grantLabel else null,
        onStatusAction = { scope.launch { granted = callbacks.requestPermission() } },
        preview = if (granted && !cameraDead) {
            {
                CameraScanner(
                    torch = torch,
                    lensFacing = lensFacing,
                    onDecoded = callbacks.onDecoded,
                    onUnavailable = { cameraDead = true },
                    modifier = Modifier.fillMaxSize(),
                )
            }
        } else {
            null
        },
    )
}
