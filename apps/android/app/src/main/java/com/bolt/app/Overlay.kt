package com.bolt.app

import android.content.Context
import android.graphics.PixelFormat
import android.provider.Settings
import android.view.Gravity
import android.view.WindowManager
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.togetherWith
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
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

/** What the floating card shows: an incoming offer, the transfer after it, or text someone sent. */
sealed interface OverlayCard {
    data class Incoming(val offer: Offer) : OverlayCard
    data class Progress(val xfer: Xfer, val more: Int) : OverlayCard
    data class Received(val text: ReceivedText, val more: Int) : OverlayCard
}

/**
 * One card as a window on top of whatever app is in front, so incoming transfers can be answered and followed
 * without opening Bolt. The card stays put and its content morphs: accept card -> progress -> done.
 */
class CardOverlay(private val ctx: Context) : LifecycleOwner, SavedStateRegistryOwner, ViewModelStoreOwner {
    private val registry = LifecycleRegistry(this)
    private val controller = SavedStateRegistryController.create(this)
    private val store = ViewModelStore()
    override val lifecycle: Lifecycle get() = registry
    override val savedStateRegistry: SavedStateRegistry get() = controller.savedStateRegistry
    override val viewModelStore: ViewModelStore get() = store

    private val wm = ctx.getSystemService(Context.WINDOW_SERVICE) as WindowManager
    private var view: ComposeView? = null
    private var kind by mutableStateOf("offer")
    private var offer by mutableStateOf<Offer?>(null)
    private var xfer by mutableStateOf<Xfer?>(null)
    private var text by mutableStateOf<ReceivedText?>(null)
    private var more by mutableStateOf(0)
    private val visibleState = MutableTransitionState(false)
    private val main = android.os.Handler(android.os.Looper.getMainLooper())
    private var pendingHide: Runnable? = null
    private var removal: Runnable? = null

    init {
        controller.performRestore(null)
        registry.currentState = Lifecycle.State.RESUMED
    }

    /** Show [card], or (null) slide the card away after a short grace so offer -> progress never flickers. */
    fun post(card: OverlayCard?) {
        pendingHide?.let { main.removeCallbacks(it) }
        pendingHide = null
        if (card == null) {
            if (view == null) return
            pendingHide = Runnable { hide() }.also { main.postDelayed(it, 700) }
            return
        }
        when (card) {
            is OverlayCard.Incoming -> { offer = card.offer; kind = "offer" }
            is OverlayCard.Progress -> { xfer = card.xfer; more = card.more; kind = "xfer" }
            is OverlayCard.Received -> { text = card.text; more = card.more; kind = "text" }
        }
        show()
    }

    private fun show() {
        removal?.let { main.removeCallbacks(it) }
        removal = null
        visibleState.targetState = true
        if (view != null) return
        val v = ComposeView(ctx).apply {
            setViewTreeLifecycleOwner(this@CardOverlay)
            setViewTreeSavedStateRegistryOwner(this@CardOverlay)
            setViewTreeViewModelStoreOwner(this@CardOverlay)
            setContent {
                val mode by Engine.themeMode.collectAsState()
                BoltTheme(mode) {
                    Box(Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = 16.dp, bottom = 2.dp), contentAlignment = Alignment.BottomCenter) {
                        AnimatedVisibility(
                            visibleState,
                            enter = slideInVertically(tween(460, easing = Motion.ease)) { it } + fadeIn(tween(300)),
                            exit = slideOutVertically(tween(340, easing = Motion.ease)) { it } + fadeOut(tween(220)),
                        ) {
                            Column(
                                Modifier.widthIn(max = 560.dp).fillMaxWidth().shadow(18.dp, RoundedCornerShape(28.dp))
                                    .clip(RoundedCornerShape(28.dp)).background(Fd.colors.surface)
                                    .padding(horizontal = Space.page, vertical = Space.l),
                            ) {
                                // the card keeps its place; only what is inside it changes
                                AnimatedContent(
                                    targetState = kind,
                                    // A plain cross-fade, no scaling and no animated resizing: the window is wrap-content, so animating the
                                    // card's size means re-laying-out a system window every frame, which is what made this stutter.
                                    transitionSpec = {
                                        (fadeIn(tween(240)) togetherWith fadeOut(tween(160))).using(SizeTransform(clip = false) { _, _ -> snap() })
                                    },
                                    label = "card",
                                ) { k ->
                                    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(Space.m)) {
                                        val o = offer; val x = xfer; val t = text
                                        when {
                                            k == "offer" && o != null -> OfferContent(o)
                                            k == "xfer" && x != null -> TransferPanelContent(x, more)
                                            k == "text" && t != null -> ReceivedTextBody(t, more)
                                        }
                                    }
                                }
                            }
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
        // A wrap-content overlay can be left at zero height until something forces a relayout (a swipe on the navigation
        // handle did it). The card grows after its first composition, so ask the window manager to measure again.
        listOf(0L, 60L, 200L, 500L, 1200L).forEach { d ->
            main.postDelayed({ if (view === v) runCatching { v.requestLayout(); wm.updateViewLayout(v, lp) } }, d)
        }
    }

    /** Slide the card away, then drop the window. */
    fun hide() {
        pendingHide?.let { main.removeCallbacks(it) }
        pendingHide = null
        if (view == null) return
        visibleState.targetState = false
        removal?.let { main.removeCallbacks(it) }
        removal = Runnable {
            view?.let { runCatching { wm.removeView(it) } }
            view = null
            offer = null; xfer = null; text = null
            removal = null
        }.also { main.postDelayed(it, 380) }
    }
}
