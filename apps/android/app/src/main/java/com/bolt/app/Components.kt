package com.bolt.app

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/** Standard easing + durations (ms). Compose scales these by the system animator scale. */
object Motion {
    const val Quick = 140
    const val Standard = 200
    const val Large = 300
    val ease = FastOutSlowInEasing
}

/** A scrolling page with a large title, consistent margins, and a readable max width on tablets. */
@Composable
fun Page(
    title: String, subtitle: String? = null, onBack: (() -> Unit)? = null,
    action: (@Composable () -> Unit)? = null, content: LazyListScope.() -> Unit,
) {
    Box(Modifier.fillMaxSize().background(Fd.colors.background), contentAlignment = Alignment.TopCenter) {
        LazyColumn(
            Modifier.widthIn(max = 640.dp).fillMaxSize(),
            contentPadding = PaddingValues(start = Space.page, end = Space.page, bottom = Space.xxl),
            verticalArrangement = Arrangement.spacedBy(Space.xs),
        ) {
            item {
                Column(Modifier.padding(top = if (onBack != null) Space.s else Space.xl, bottom = Space.m)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        if (onBack != null) {
                            // the glyph sits on the page margin; the box is narrower than the touch target so the title stays close
                            Box(Modifier.size(width = 36.dp, height = 48.dp)) {
                                androidx.compose.material3.IconButton(onBack, Modifier.offset(x = (-12).dp)) {
                                    Icon(androidx.compose.material.icons.Icons.AutoMirrored.Filled.ArrowBack, "Back", tint = Fd.colors.text)
                                }
                            }
                        }
                        Text(title, Modifier.weight(1f).semantics { heading() }, style = MaterialTheme.typography.headlineLarge, color = Fd.colors.text, maxLines = 1)
                        action?.invoke()
                    }
                    if (subtitle != null) {
                        Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = Fd.colors.textSecondary, modifier = Modifier.padding(top = Space.xs))
                    }
                }
            }
            content()
        }
    }
}

@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text.uppercase(), modifier.padding(start = Space.l, top = Space.xl, bottom = Space.s).semantics { heading() },
        style = MaterialTheme.typography.labelSmall, color = Fd.colors.textSecondary,
    )
}

/** Inset grouped container (rows separated by hairlines). */
@Composable
fun Group(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier.fillMaxWidth().clip(Radius.group).background(Fd.colors.surface).animateContentSize(tween(Motion.Standard, easing = Motion.ease)),
        content = content,
    )
}

@Composable
fun GroupDivider(inset: Dp = Space.l) {
    HorizontalDivider(Modifier.padding(start = inset), thickness = 0.5.dp, color = Fd.colors.separator)
}

/** One row: optional leading, title + subtitle, optional trailing. Whole row is the touch target. */
@Composable
fun ListRow(
    title: String,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    leading: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
    onClick: (() -> Unit)? = null,
    titleColor: Color = Fd.colors.text,
    maxSubtitleLines: Int = 2,
) {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val bg by animateColorAsState(if (pressed) Fd.colors.surfaceRaised else Color.Transparent, tween(Motion.Quick), label = "row")
    Row(
        modifier.fillMaxWidth().background(bg)
            .let { if (onClick != null) it.clickable(source, null, role = Role.Button, onClick = onClick) else it }
            .heightIn(min = 56.dp).padding(horizontal = Space.l, vertical = Space.m),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.m),
    ) {
        leading?.invoke()
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = titleColor, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (subtitle != null) Text(subtitle, Modifier.padding(top = 2.dp), style = MaterialTheme.typography.bodyMedium, color = Fd.colors.textSecondary, maxLines = maxSubtitleLines, overflow = TextOverflow.Ellipsis)
        }
        trailing?.invoke()
    }
}

/** Free-form padded block inside a Group (for text, fields, buttons). */
@Composable
fun Block(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(modifier.fillMaxWidth().padding(Space.l), verticalArrangement = Arrangement.spacedBy(Space.m), content = content)
}

enum class BtnStyle { Success, Primary, Secondary, Plain, Destructive, Danger }

/** Buttons with a subtle press-scale; 50dp tall so the touch target is comfortable. */
@Composable
fun FdButton(
    text: String, onClick: () -> Unit, modifier: Modifier = Modifier,
    style: BtnStyle = BtnStyle.Primary, enabled: Boolean = true, icon: ImageVector? = null, fill: Boolean = true,
    height: Dp = Space.controlHeight,
) {
    val c = Fd.colors
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed && enabled) 0.97f else 1f, tween(Motion.Quick, easing = Motion.ease), label = "press")
    val (bg, fg) = when (style) {
        BtnStyle.Primary -> c.accent to c.onAccent
        BtnStyle.Secondary -> c.accentSoft to c.accent
        BtnStyle.Plain -> Color.Transparent to c.accent
        BtnStyle.Success -> c.good.copy(alpha = 0.16f) to c.good
        // one red for everything destructive or refusing (Decline, Deny, Cancel, Clear, Stop): solid #FF4146 with white text
        BtnStyle.Destructive, BtnStyle.Danger -> Color(0xFFFF4146) to Color.White
    }
    val alpha = if (enabled) 1f else 0.38f
    Row(
        modifier.let { if (fill) it.fillMaxWidth() else it }.height(height).scale(scale)
            .clip(RoundedCornerShape(height / 2)).background(bg.copy(alpha = bg.alpha * alpha))
            .clickable(source, null, enabled = enabled, role = Role.Button, onClick = onClick).padding(horizontal = Space.l),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.Center,
    ) {
        if (icon != null) {
            Icon(icon, null, tint = fg.copy(alpha = alpha), modifier = Modifier.size(18.dp))
            Spacer(Modifier.size(Space.s))
        }
        Text(text, style = MaterialTheme.typography.labelLarge, color = fg.copy(alpha = alpha), maxLines = 1)
    }
}

@Composable
fun FdField(
    value: String, onChange: (String) -> Unit, placeholder: String, modifier: Modifier = Modifier,
    singleLine: Boolean = true, maxLines: Int = if (singleLine) 1 else 4, isError: Boolean = false,
) {
    val c = Fd.colors
    OutlinedTextField(
        value, onChange, modifier.fillMaxWidth().heightIn(min = Space.controlHeight), singleLine = singleLine, maxLines = maxLines, isError = isError,
        placeholder = { Text(placeholder, color = c.textTertiary) }, shape = Radius.control, textStyle = MaterialTheme.typography.bodyLarge,
        colors = OutlinedTextFieldDefaults.colors(
            focusedTextColor = c.text, unfocusedTextColor = c.text, cursorColor = c.accent,
            focusedBorderColor = c.accent, unfocusedBorderColor = c.separator, errorBorderColor = c.bad,
            focusedContainerColor = c.background, unfocusedContainerColor = c.background,
        ),
    )
}

/** Neutral initial avatar: identity comes from the name, not from random colours. */
@Composable
fun Avatar(name: String, size: Dp = 40.dp) {
    Box(Modifier.size(size).clip(CircleShape).background(Fd.colors.accent), contentAlignment = Alignment.Center) {
        Text((name.firstOrNull() ?: '?').uppercase(), style = MaterialTheme.typography.titleMedium, color = Fd.colors.onAccent)
    }
}

@Composable
fun StatusDot(color: Color, desc: String) {
    Box(Modifier.size(8.dp).clip(CircleShape).background(color).semantics { contentDescription = desc })
}

@Composable
fun ThinProgress(fraction: Float?, modifier: Modifier = Modifier) {
    val c = Fd.colors
    val animated by animateFloatAsState(fraction ?: 0f, tween(Motion.Standard, easing = Motion.ease), label = "progress")
    Box(modifier.fillMaxWidth().height(4.dp).clip(CircleShape).background(c.separator)) {
        if (fraction != null) Box(Modifier.fillMaxWidth(animated.coerceIn(0f, 1f)).height(4.dp).clip(CircleShape).background(c.accent))
        else androidx.compose.material3.LinearProgressIndicator(Modifier.fillMaxSize(), color = c.accent, trackColor = Color.Transparent)
    }
}

@Composable
fun EmptyState(icon: ImageVector, title: String, body: String, modifier: Modifier = Modifier) {
    Column(modifier.fillMaxWidth().padding(vertical = Space.xxl, horizontal = Space.l), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(Space.s)) {
        Icon(icon, null, tint = Fd.colors.textTertiary, modifier = Modifier.size(32.dp))
        Text(title, style = MaterialTheme.typography.titleMedium, color = Fd.colors.text, textAlign = TextAlign.Center)
        Text(body, style = MaterialTheme.typography.bodyMedium, color = Fd.colors.textSecondary, textAlign = TextAlign.Center)
    }
}

/** Bottom sheet for decisions that need an answer (incoming offer, pairing request). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DecisionSheet(onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    ModalBottomSheet(
        onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        shape = Radius.sheet, containerColor = Fd.colors.surfaceRaised, contentColor = Fd.colors.text, tonalElevation = 0.dp,
    ) {
        Column(Modifier.fillMaxWidth().padding(horizontal = Space.page).padding(bottom = Space.xl), verticalArrangement = Arrangement.spacedBy(Space.m), content = content)
    }
}
