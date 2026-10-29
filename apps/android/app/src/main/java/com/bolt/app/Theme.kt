package com.bolt.app

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat

/**
 * Design tokens. One neutral ramp, one restrained accent, and semantic colours that
 * appear only where they carry meaning (success / warning / failure).
 */
class FdPalette(
    val dark: Boolean,
    val top: Color,             // home header area (darkest / lightest)
    val background: Color,      // lower panel and sub-pages
    val surface: Color,         // grouped content
    val surfaceRaised: Color,   // sheets, pressed rows
    val separator: Color,       // hairlines
    val text: Color,
    val textSecondary: Color,
    val textTertiary: Color,
    val accent: Color,
    val onAccent: Color,
    val accentSoft: Color,      // tinted fills behind accent content
    val good: Color,
    val warn: Color,
    val bad: Color,
)

/** Near-black neutrals with one lime accent (#D7F62F). Dark is the default. */
private val Light = FdPalette(
    dark = false, top = Color(0xFFF2F5FA),
    background = Color(0xFFF2F5FA), surface = Color(0xFFFFFFFF), surfaceRaised = Color(0xFFFFFFFF),
    separator = Color(0x1A0B1B33), text = Color(0xFF0B1B33), textSecondary = Color(0x990B1B33), textTertiary = Color(0x590B1B33),
    accent = Color(0xFFD7F62F), onAccent = Color(0xFF14170A), accentSoft = Color(0x24D7F62F),
    good = Color(0xFF24A33C), warn = Color(0xFFB25E00), bad = Color(0xFFD70015),
)

private val Dark = FdPalette(
    dark = true, top = Color(0xFF000000),
    background = Color(0xFF121214), surface = Color(0xFF1C1C1F), surfaceRaised = Color(0xFF28282C),
    separator = Color(0x26FFFFFF), text = Color(0xFFF5F5F7), textSecondary = Color(0xFF9B9BA3), textTertiary = Color(0xFF66666E),
    accent = Color(0xFFD7F62F), onAccent = Color(0xFF14170A), accentSoft = Color(0x33D7F62F),
    good = Color(0xFF32D74B), warn = Color(0xFFFFB454), bad = Color(0xFFFF6B7A),
)

val LocalPalette = staticCompositionLocalOf { Light }

object Fd {
    val colors: FdPalette @Composable @ReadOnlyComposable get() = LocalPalette.current
}

/** Spacing scale (dp) and radii used everywhere instead of ad-hoc numbers. */
object Space {
    val xs = 4.dp; val s = 8.dp; val m = 12.dp; val l = 16.dp; val xl = 20.dp; val xxl = 28.dp
    val page = 20.dp                 // horizontal page margin
    val controlHeight = 50.dp        // primary buttons / fields (>= 48dp touch target)
}

object Radius {
    val control = RoundedCornerShape(14.dp)
    val group = RoundedCornerShape(20.dp)
    val sheet = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp)
}

private fun scheme(p: FdPalette): ColorScheme {
    val base = if (p.dark) darkColorScheme() else lightColorScheme()
    return base.copy(
        primary = p.accent, onPrimary = p.onAccent, primaryContainer = p.accentSoft, onPrimaryContainer = p.accent,
        secondary = p.accent, background = p.background, onBackground = p.text,
        surface = p.surface, onSurface = p.text, surfaceVariant = p.surfaceRaised, onSurfaceVariant = p.textSecondary,
        surfaceContainer = p.surface, surfaceContainerHigh = p.surfaceRaised, surfaceContainerHighest = p.surfaceRaised,
        surfaceContainerLow = p.surface, surfaceContainerLowest = p.top,
        outline = p.separator, outlineVariant = p.separator, error = p.bad, onError = Color.White,
        scrim = Color(0x66000000),
    )
}

@OptIn(androidx.compose.ui.text.ExperimentalTextApi::class)
private fun manrope(w: Int) = androidx.compose.ui.text.font.Font(
    R.font.manrope, FontWeight(w), variationSettings = androidx.compose.ui.text.font.FontVariation.Settings(androidx.compose.ui.text.font.FontVariation.weight(w)),
)

/** Manrope (SIL OFL, bundled) — the same geometric family blip.net uses for headings. */
private val sans = FontFamily(manrope(400), manrope(500), manrope(600), manrope(700), manrope(800))

/** Type scale: one large title per screen, quiet everything else. */
private val type = Typography(
    headlineLarge = TextStyle(fontFamily = sans, fontWeight = FontWeight.ExtraBold, fontSize = 26.sp, lineHeight = 32.sp, letterSpacing = (-0.5).sp),
    headlineMedium = TextStyle(fontFamily = sans, fontWeight = FontWeight.Bold, fontSize = 26.sp, lineHeight = 32.sp, letterSpacing = (-0.4).sp),
    titleLarge = TextStyle(fontFamily = sans, fontWeight = FontWeight.SemiBold, fontSize = 20.sp, lineHeight = 26.sp, letterSpacing = (-0.2).sp),
    titleMedium = TextStyle(fontFamily = sans, fontWeight = FontWeight.SemiBold, fontSize = 16.sp, lineHeight = 21.sp, letterSpacing = (-0.1).sp),
    bodyLarge = TextStyle(fontFamily = sans, fontSize = 16.sp, lineHeight = 22.sp),
    bodyMedium = TextStyle(fontFamily = sans, fontSize = 14.sp, lineHeight = 20.sp),
    bodySmall = TextStyle(fontFamily = sans, fontSize = 12.sp, lineHeight = 16.sp),
    labelLarge = TextStyle(fontFamily = sans, fontWeight = FontWeight.SemiBold, fontSize = 16.sp, lineHeight = 21.sp),
    labelMedium = TextStyle(fontFamily = sans, fontWeight = FontWeight.Medium, fontSize = 13.sp, lineHeight = 18.sp, letterSpacing = 0.1.sp),
    labelSmall = TextStyle(fontFamily = sans, fontWeight = FontWeight.Medium, fontSize = 11.sp, lineHeight = 14.sp, letterSpacing = 0.2.sp),
)

private val shapes = Shapes(
    extraSmall = RoundedCornerShape(8.dp), small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(16.dp), large = RoundedCornerShape(20.dp), extraLarge = RoundedCornerShape(28.dp),
)

/**
 * @param mode "system" | "light" | "dark". Motion durations follow the system
 * "Animator duration scale" automatically (Compose reads it), so turning animations
 * off in Developer options / accessibility removes them here too.
 */
@Composable
fun BoltTheme(mode: String = "system", content: @Composable () -> Unit) {
    val dark = when (mode) { "dark" -> true; "light" -> false; else -> isSystemInDarkTheme() }
    val palette = if (dark) Dark else Light
    val view = LocalView.current
    // the status/navigation bar tint only applies inside an Activity (the over-other-apps card has none)
    val activity = view.context as? Activity
    if (!view.isInEditMode && activity != null) {
        SideEffect {
            val window = activity.window
            WindowCompat.getInsetsController(window, view).apply {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
    }
    CompositionLocalProvider(LocalPalette provides palette) {
        MaterialTheme(colorScheme = scheme(palette), typography = type, shapes = shapes, content = content)
    }
}
