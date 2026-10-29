package com.bolt.app

import android.util.Size
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size as GSize
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.EncodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/** Draws [text] as a QR code (dark modules on a white card so any scanner reads it). */
@Composable
fun QrCode(text: String, modifier: Modifier = Modifier) {
    val matrix = remember(text) {
        QRCodeWriter().encode(
            text, BarcodeFormat.QR_CODE, 0, 0,
            mapOf(EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M, EncodeHintType.MARGIN to 0),
        )
    }
    Box(modifier.clip(RoundedCornerShape(20.dp)).background(Color.White).padding(16.dp).aspectRatio(1f)) {
        Canvas(Modifier.fillMaxWidth().aspectRatio(1f)) {
            val n = matrix.width
            val cell = size.width / n
            for (y in 0 until n) for (x in 0 until n) {
                if (matrix[x, y]) drawRoundRect(
                    Color(0xFF0B1B33), Offset(x * cell, y * cell), GSize(cell + 0.5f, cell + 0.5f), CornerRadius(cell * 0.18f),
                )
            }
        }
    }
}

/** Live camera preview that reports the first QR code containing text. */
@Composable
fun QrScanner(modifier: Modifier = Modifier, onResult: (String) -> Unit) {
    val ctx = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val executor = remember { Executors.newSingleThreadExecutor() }
    val fired = remember { AtomicBoolean(false) }
    val reader = remember {
        MultiFormatReader().apply {
            setHints(mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE), DecodeHintType.TRY_HARDER to true))
        }
    }
    DisposableEffect(Unit) { onDispose { executor.shutdown() } }
    AndroidView(
        modifier = modifier.clip(RoundedCornerShape(20.dp)),
        factory = { c ->
            val view = PreviewView(c).apply { scaleType = PreviewView.ScaleType.FILL_CENTER }
            val future = ProcessCameraProvider.getInstance(c)
            future.addListener({
                val provider = future.get()
                val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
                val analysis = ImageAnalysis.Builder()
                    .setTargetResolution(Size(1280, 720))
                    .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                    .build()
                analysis.setAnalyzer(executor) { img -> decode(img, reader) { text -> if (fired.compareAndSet(false, true)) ContextCompat.getMainExecutor(c).execute { onResult(text) } } }
                runCatching {
                    provider.unbindAll()
                    provider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
                }
            }, ContextCompat.getMainExecutor(c))
            view
        },
    )
}

private fun decode(img: ImageProxy, reader: MultiFormatReader, found: (String) -> Unit) {
    try {
        val plane = img.planes[0]
        val buf = plane.buffer
        val data = ByteArray(buf.remaining()).also { buf.get(it) }
        val rowStride = plane.rowStride
        val src = PlanarYUVLuminanceSource(data, rowStride, img.height, 0, 0, img.width, img.height, false)
        val result = reader.decodeWithState(BinaryBitmap(HybridBinarizer(src)))
        found(result.text)
    } catch (_: Exception) {
        // no QR in this frame
    } finally {
        reader.reset()
        img.close()
    }
}
