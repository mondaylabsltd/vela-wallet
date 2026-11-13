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
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LifecycleResumeEffect
import app.getvela.wallet.core.platform.AppSettings
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
    /** r14-10: the same button's words once it opens this app's Settings page (a refusal for good). */
    val settingsLabel: String,
    val noQrFound: String,
    val cameraUnavailable: String,
    val decodeFailed: String,
    /**
     * Whether the system would still show its camera dialog (Android's
     * `shouldShowRequestPermissionRationale`); `null` asks the activity.
     */
    val showRationale: (() -> Boolean)? = null,
    /** Opens this app's page in Settings; `null` opens it from the context. */
    val openSettings: (() -> Unit)? = null,
)

/**
 * The drawn scanner with a camera behind its brackets: permission first
 * (asked once on entry, re-asked from the status line), the frames decoded
 * by [QrDecoder], the photo tool through the picker, torch on its tool.
 */
@Composable
fun LiveScanSurface(
    model: ScanModel,
    callbacks: ScanCallbacks,
    modifier: Modifier = Modifier,
    /** The host's word about the last code read (a code it cannot use), on the status line. */
    message: String? = null,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var granted by remember {
        mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED)
    }
    var asked by remember { mutableStateOf(false) }
    // r14-10: bumped after each answer and each return to the front, so what
    // the button does (and says) is read again — the rationale flag moves
    // with every refusal and with a visit to Settings.
    var refusedFor by remember { mutableStateOf(0) }
    var torch by remember { mutableStateOf(false) }
    var status by remember { mutableStateOf<String?>(null) }
    var cameraDead by remember { mutableStateOf(false) }
    // Spec 048: 翻转 — the other camera; CameraScanner falls back to the back one when there is no front camera.
    var lensFacing by remember { mutableStateOf(CameraSelector.LENS_FACING_BACK) }
    val haptic = rememberVelaHaptic()
    // A gallery, board, preview or developer-route session opens no camera
    // (`ScanCamera`): it does not ask for one either — the frame is the
    // fixture's, whatever the permission says.
    val fixture = ScanCamera.fixtureOnly(inspection = androidx.compose.ui.platform.LocalInspectionMode.current)
    LaunchedEffect(Unit) {
        if (!fixture && !granted && !asked) {
            asked = true
            granted = callbacks.requestPermission()
            refusedFor += 1
        }
    }
    // Back from Settings (or anywhere the person may have turned the camera
    // on): read the permission again rather than keep the refusal on screen.
    LifecycleResumeEffect(Unit) {
        if (!fixture && !granted) {
            granted = ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
            refusedFor += 1
        }
        onPauseOrDispose {}
    }
    val showRationale: () -> Boolean = callbacks.showRationale ?: {
        AppSettings.activityOf(context)
            ?.let { ActivityCompat.shouldShowRequestPermissionRationale(it, Manifest.permission.CAMERA) }
            ?: true
    }
    val openSettings: () -> Unit = callbacks.openSettings ?: { AppSettings.open(context) }
    // r14-10: what the button does now — and so what it says.
    val grantAction = remember(asked, granted, refusedFor) {
        CameraRefusal.grantAction(askedBefore = asked, showRationale = showRationale())
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
            fixture -> message ?: status
            !granted -> callbacks.permissionText
            cameraDead -> callbacks.cameraUnavailable
            else -> message ?: status
        },
        statusAction = if (!granted && !fixture) CameraRefusal.label(grantAction, callbacks.grantLabel, callbacks.settingsLabel) else null,
        // A camera refused for good cannot be asked for again — the system
        // shows no dialog — so the same button opens this app's Settings
        // page, as iOS's does (NATIVE-LAUNCH-CHECKLIST C).
        onStatusAction = {
            when (CameraRefusal.grantAction(askedBefore = asked, showRationale = showRationale())) {
                GrantAction.Ask -> scope.launch {
                    asked = true
                    granted = callbacks.requestPermission()
                    refusedFor += 1
                }
                GrantAction.OpenSettings -> openSettings()
            }
        },
        preview = if (fixture || (granted && !cameraDead)) {
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
