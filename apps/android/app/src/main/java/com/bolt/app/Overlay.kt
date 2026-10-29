package com.bolt.app

import android.content.Context
import android.graphics.PixelFormat
import android.provider.Settings
import android.view.Gravity
import android.view.WindowManager
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import androidx.lifecycle.setViewTreeLifecycleOwner
import androidx.lifecycle.setViewTreeViewModelStoreOwner
import androidx.savedstate.SavedStateRegistry
import androidx.savedstate.SavedStateRegistryController
import androidx.savedstate.SavedStateRegistryOwner
import androidx.savedstate.setViewTreeSavedStateRegistryOwner

/** True when the user allowed "Display over other apps" for Bolt. */
fun canShowOverOtherApps(ctx: Context) = Settings.canDrawOverlays(ctx)

/**
 * The accept / decline card as a window on top of whatever app is in front, so an incoming transfer can be
 * answered without opening Bolt. It is the same [OfferContent] as the home screen, in the same panel style.
 */
class OfferOverlay(private val ctx: Context) : LifecycleOwner, SavedStateRegistryOwner, ViewModelStoreOwner {
    private val registry = LifecycleRegistry(this)
    private val controller = SavedStateRegistryController.create(this)
    private val store = ViewModelStore()
    override val lifecycle: Lifecycle get() = registry
    override val savedStateRegistry: SavedStateRegistry get() = controller.savedStateRegistry
    override val viewModelStore: ViewModelStore get() = store

    private val wm = ctx.getSystemService(Context.WINDOW_SERVICE) as WindowManager
    private var view: ComposeView? = null
    private var offer by mutableStateOf<Offer?>(null)
    private val visibleState = MutableTransitionState(false)
    private val main = android.os.Handler(android.os.Looper.getMainLooper())
    private var removal: Runnable? = null

    init {
        controller.performRestore(null)
        registry.currentState = Lifecycle.State.RESUMED
    }

    fun show(o: Offer) {
        offer = o
        removal?.let { main.removeCallbacks(it) }
        removal = null
        visibleState.targetState = true
        if (view != null) return
        val v = ComposeView(ctx).apply {
            setViewTreeLifecycleOwner(this@OfferOverlay)
            setViewTreeSavedStateRegistryOwner(this@OfferOverlay)
            setViewTreeViewModelStoreOwner(this@OfferOverlay)
            setContent {
                val mode by Engine.themeMode.collectAsState()
                BoltTheme(mode) {
                    val shown = offer ?: return@BoltTheme
                    Box(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 16.dp), contentAlignment = Alignment.BottomCenter) {
                        AnimatedVisibility(
                            visibleState,
                            enter = slideInVertically(tween(460, easing = Motion.ease)) { it } + fadeIn(tween(300)),
                            exit = slideOutVertically(tween(340, easing = Motion.ease)) { it } + fadeOut(tween(220)),
                        ) {
                            Column(
                                Modifier.widthIn(max = 560.dp).fillMaxWidth().shadow(18.dp, RoundedCornerShape(28.dp))
                                    .clip(RoundedCornerShape(28.dp)).background(Fd.colors.surface)
                                    .padding(horizontal = Space.page, vertical = Space.l),
                            ) { OfferContent(shown) }
                        }
                    }
                }
            }
        }
        val lp = WindowManager.LayoutParams(
            WindowManager.LayoutParams.MATCH_PARENT, WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
            PixelFormat.TRANSLUCENT,
        ).apply { gravity = Gravity.BOTTOM }
        runCatching { wm.addView(v, lp); view = v }
    }

    /** Slide the card away, then drop the window. */
    fun hide() {
        if (view == null) return
        visibleState.targetState = false
        removal?.let { main.removeCallbacks(it) }
        removal = Runnable {
            view?.let { runCatching { wm.removeView(it) } }
            view = null
            offer = null
            removal = null
        }.also { main.postDelayed(it, 380) }
    }
}
