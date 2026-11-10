package com.pairspan.app.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Pairspan design tokens: the website palette (docs/design.md).
 * Ember is the only action color; Mint means "connected"; Ink cards carry the hero.
 */
object Brand {
    val Ink = Color(0xFF161616)
    val Creme = Color(0xFFFAF4EC)
    val OffWhite = Color(0xFFFFFEF7)
    val Taupe = Color(0xFFEAE4D8)
    val Muted = Color(0xFF5C574F)
    val Ember = Color(0xFFFA5424)
    val EmberSoft = Color(0xFFFC997B)
    val Mint = Color(0xFF33C791)
    val MintSoft = Color(0xFF73E2B6)
    val Sky = Color(0xFF0D8DFF)
    val Stone = Color(0xFFC7C2B8)

    val NightBg = Color(0xFF111110)
    val NightSurface = Color(0xFF1C1B19)
    val NightRaised = Color(0xFF2C2A27)
    val NightText = Color(0xFFF6F1E8)
    val NightMuted = Color(0xFFA8A29E)
    val EmberNight = Color(0xFFFF6A3D)
}

/** Colors Material doesn't name. */
@Immutable
data class StatusColors(val online: Color, val offline: Color, val action: Color, val hero: Color, val onHero: Color, val heroMuted: Color)

val LocalStatus = staticCompositionLocalOf {
    StatusColors(Brand.Mint, Brand.Stone, Brand.Ember, Brand.Ink, Brand.OffWhite, Brand.Stone)
}

private val Light = lightColorScheme(
    primary = Brand.Ember, onPrimary = Brand.OffWhite,
    primaryContainer = Brand.EmberSoft, onPrimaryContainer = Brand.Ink,
    secondary = Brand.Ink, onSecondary = Brand.OffWhite,
    tertiary = Brand.Mint, onTertiary = Brand.Ink,
    background = Brand.Creme, onBackground = Brand.Ink,
    surface = Brand.OffWhite, onSurface = Brand.Ink,
    surfaceVariant = Brand.Taupe, onSurfaceVariant = Brand.Muted,
    surfaceContainer = Brand.OffWhite, surfaceContainerHigh = Brand.OffWhite, surfaceContainerHighest = Brand.Taupe,
    outline = Brand.Stone, outlineVariant = Brand.Taupe,
    error = Color(0xFFB42318), onError = Brand.OffWhite,
)

private val Dark = darkColorScheme(
    primary = Brand.EmberNight, onPrimary = Brand.Ink,
    primaryContainer = Color(0xFF5A2210), onPrimaryContainer = Brand.EmberSoft,
    secondary = Brand.NightText, onSecondary = Brand.Ink,
    tertiary = Brand.MintSoft, onTertiary = Brand.Ink,
    background = Brand.NightBg, onBackground = Brand.NightText,
    surface = Brand.NightSurface, onSurface = Brand.NightText,
    surfaceVariant = Brand.NightRaised, onSurfaceVariant = Brand.NightMuted,
    surfaceContainer = Brand.NightSurface, surfaceContainerHigh = Brand.NightSurface, surfaceContainerHighest = Brand.NightRaised,
    outline = Color(0xFF3A3834), outlineVariant = Brand.NightRaised,
    error = Color(0xFFFF8A73), onError = Brand.Ink,
)

/** Tight, heavy headlines like the website; relaxed body text. */
private val Type = Typography(
    displaySmall = TextStyle(fontSize = 38.sp, lineHeight = 40.sp, fontWeight = FontWeight.ExtraBold, letterSpacing = (-1.8).sp),
    headlineMedium = TextStyle(fontSize = 30.sp, lineHeight = 33.sp, fontWeight = FontWeight.ExtraBold, letterSpacing = (-1.3).sp),
    headlineSmall = TextStyle(fontSize = 24.sp, lineHeight = 28.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.9).sp),
    titleLarge = TextStyle(fontSize = 20.sp, lineHeight = 24.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.5).sp),
    titleMedium = TextStyle(fontSize = 16.sp, lineHeight = 21.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.2).sp),
    bodyLarge = TextStyle(fontSize = 16.sp, lineHeight = 23.sp),
    bodyMedium = TextStyle(fontSize = 14.sp, lineHeight = 20.sp),
    bodySmall = TextStyle(fontSize = 12.5.sp, lineHeight = 17.sp),
    labelLarge = TextStyle(fontSize = 15.sp, fontWeight = FontWeight.Bold, letterSpacing = (-0.1).sp),
    labelMedium = TextStyle(fontSize = 11.5.sp, fontWeight = FontWeight.ExtraBold, letterSpacing = 0.7.sp),
)

private val Shape = Shapes(
    small = RoundedCornerShape(12.dp),
    medium = RoundedCornerShape(18.dp),
    large = RoundedCornerShape(26.dp),
    extraLarge = RoundedCornerShape(36.dp),
)

@Composable
fun PairspanTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    val status = if (dark) {
        StatusColors(Brand.MintSoft, Brand.NightMuted, Brand.EmberNight, Brand.NightRaised, Brand.NightText, Brand.NightMuted)
    } else {
        StatusColors(Brand.Mint, Brand.Stone, Brand.Ember, Brand.Ink, Brand.OffWhite, Brand.Stone)
    }
    androidx.compose.runtime.CompositionLocalProvider(LocalStatus provides status) {
        MaterialTheme(colorScheme = if (dark) Dark else Light, typography = Type, shapes = Shape, content = content)
    }
}
