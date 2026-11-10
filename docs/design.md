# Pairspan design

One palette and one set of rules for the website, the desktop app and the Android app.

| Where | Source of truth |
|---|---|
| Website | `apps/web/app/globals.css` (`:root` tokens) |
| Desktop | `apps/desktop/ui/index.html` (same tokens) |
| Android | `apps/android/app/src/main/java/com/pairspan/app/ui/Theme.kt` (`Brand`, light and dark schemes) |
| Icons | `assets/brand/*.svg` (render with `rsvg-convert`) |

## Colors

| Token | Light | Dark | Use |
|---|---|---|---|
| `creme` / background | `#FAF4EC` | `#111110` | Page and window background, also behind the Android system bars |
| `off-white` / surface | `#FFFEF7` | `#1C1B19` | Cards, dialogs |
| `taupe` / line | `#EAE4D8` | `#2C2A27` | Borders, dividers, tonal buttons |
| `ink` / text | `#161616` | `#F6F1E8` | Text; also the **hero card** background in light mode |
| `muted` | `#5C574F` | `#A8A29E` | Secondary text |
| `ember` / primary | `#FA5424` | `#FF6A3D` | The only action color: primary buttons, links, highlights, "needs action" dots |
| `ember-soft` | `#FC997B` | — | Gradient partner of ember (icon), soft accents |
| `mint` | `#33C791` | `#73E2B6` | Connected / done. Never used for buttons |
| `sky` | `#0D8DFF` | — | Reserved for informational accents |

Rules:

1. **One accent.** Ember marks what you can do; mint marks what is working. Don't add a third accent.
2. **Dark hero, light page.** The top status card is ink with off-white text (a raised dark surface at night).
3. **Status = dot.** 10 px circle; mint with a 25 % halo when live, stone/muted when offline, ember when the user must act.
4. Text on ember is off-white; text on mint is ink.

## Type

System fonts everywhere (SF Pro, Roboto, Segoe UI). Headlines are heavy and tight, body text is relaxed.

| Role | Size | Weight | Tracking |
|---|---|---|---|
| Display (web hero) | clamp(3rem, 7vw, 5.8rem) | 700 | −0.055em |
| Section title | 30–36 | 800 | −0.045em |
| Card title | 16–20 | 700 | −0.02em |
| Body | 14–16 | 400 | 0 |
| Label / kicker | 11.5–12.5, uppercase | 800 | +0.06em |
| Code | SF Mono / Menlo | 400 | 0 |

## Shape and space

- Radii: 12 (inputs), 18 (dialogs), **26 (cards)**, 36 (large panels), pill (buttons, chips).
- Spacing steps: 4, 8, 12, 16, 20, 24, 32. Screen margin 20 dp on Android, 24–28 px on desktop/web.
- Buttons are pills. Primary = ember filled; secondary = outlined (1–1.5 px ink or line color); tertiary = text in ember.

## Android specifics

- Edge-to-edge: status and navigation bars are fully transparent (`SystemBarStyle.auto(transparent, transparent)`, contrast scrims off); the background color runs behind them and lists scroll under them with inset padding.
- Brand colors are fixed (no Material You dynamic color) so the app matches the website.
- Launcher icon: adaptive, ink background, off-white + ember link mark; monochrome layer for themed icons.

## Icon

The link mark: two interlocking strokes (off-white and an ember gradient `#FC997B → #FA5424`) around an ember dot, on an ink rounded square. Menu-bar icon is the same mark as a black template image.
