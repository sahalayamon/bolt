package com.bolt.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.collectAsState
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import kotlin.concurrent.thread

class MainActivity : ComponentActivity() {
    private var incomingPairUri by mutableStateOf<String?>(null)

    private val notifPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        // no grey scrim behind 3-button navigation: the panel colour shows through
        if (Build.VERSION.SDK_INT >= 29) window.isNavigationBarContrastEnforced = false
        super.onCreate(savedInstanceState)
        Engine.init(this)
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) notifPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        TransferService.start(this)
        // the over-other-apps card needs a one-time system switch: take the user there once, on first launch of this version
        val sp = getSharedPreferences("bolt", MODE_PRIVATE)
        if (!canShowOverOtherApps(this) && !sp.getBoolean("overlay_asked", false)) {
            sp.edit().putBoolean("overlay_asked", true).apply()
            openOverlaySettings(this)
        }
        handleIntent(intent)
        setContent {
            val themeMode by Engine.themeMode.collectAsState()
            BoltTheme(themeMode) {
                val ctx = this
                val pickFiles = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
                    stageUris(uris)
                }
                val pickPhotos = rememberLauncherForActivityResult(ActivityResultContracts.PickMultipleVisualMedia()) { uris ->
                    stageUris(uris)
                }
                val pickFolder = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
                    if (tree != null) thread {
                        runCatching { contentResolver.takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
                        Engine.stage(Saf.expandTree(ctx, tree))
                    }
                }
                val pickDest = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { tree ->
                    if (tree != null) {
                        runCatching {
                            contentResolver.takePersistableUriPermission(
                                tree, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
                            )
                        }
                        Engine.destTree = tree.toString()
                    }
                }
                val picks = Pickers(
                    files = { pickFiles.launch(arrayOf("*/*")) },
                    photos = { pickPhotos.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageAndVideo)) },
                    folder = { pickFolder.launch(null) },
                    destination = { pickDest.launch(null) },
                )
                val wantsPicker by Engine.pickerRequest.collectAsState()
                LaunchedEffect(wantsPicker) {
                    if (wantsPicker) { Engine.pickerRequest.value = false; picks.files() }
                }
                val pair = incomingPairUri
                LaunchedEffect(pair) {
                    if (pair != null) { Engine.pairWith(pair); incomingPairUri = null }
                }
                BoltApp(picks)
            }
        }
    }

    // "in front" means resumed, not merely started: the moment you switch apps, open recents or pull a pop-up over
    // Bolt it pauses (onStop can lag well behind), and that is when an incoming request should float over the other app.
    override fun onResume() {
        super.onResume()
        Engine.appVisible.value = true
        Engine.nudgeDiscovery()
    }

    override fun onPause() {
        Engine.appVisible.value = false
        super.onPause()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun stageUris(uris: List<Uri>) {
        thread { Engine.stage(uris.mapNotNull { Saf.describe(this, it) }) }
    }

    /** Share-sheet (SEND / SEND_MULTIPLE) and bolt://pair links. */
    private fun handleIntent(i: Intent?) {
        i ?: return
        when (i.action) {
            BoltWidget.ACTION_PICK_FILES -> Engine.pickerRequest.value = true
            Intent.ACTION_SEND -> {
                @Suppress("DEPRECATION")
                val uri = if (Build.VERSION.SDK_INT >= 33) i.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java) else i.getParcelableExtra(Intent.EXTRA_STREAM)
                if (uri != null) stageUris(listOf(uri))
            }
            Intent.ACTION_SEND_MULTIPLE -> {
                @Suppress("DEPRECATION")
                val list = if (Build.VERSION.SDK_INT >= 33) i.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java) else i.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
                if (!list.isNullOrEmpty()) stageUris(list)
            }
            Intent.ACTION_VIEW -> i.dataString?.takeIf { it.startsWith("bolt://pair") }?.let { incomingPairUri = it }
        }
    }
}

class Pickers(val files: () -> Unit, val photos: () -> Unit, val folder: () -> Unit, val destination: () -> Unit)
