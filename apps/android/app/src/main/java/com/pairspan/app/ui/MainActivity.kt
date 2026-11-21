package com.pairspan.app.ui

import android.Manifest
import android.annotation.SuppressLint
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.pairspan.app.BuildConfig
import com.pairspan.app.OtaUpdateManager
import com.pairspan.app.R
import com.pairspan.app.UpdateReleaseInfo
import com.pairspan.app.UpdateStatus
import com.pairspan.app.link.LinkService
import com.pairspan.app.link.LinkState
import com.pairspan.app.link.NetState
import com.pairspan.app.link.PairStore
import com.pairspan.app.link.PairingState
import com.pairspan.app.link.PcView
import kotlin.concurrent.thread

class MainActivity : ComponentActivity() {
    private lateinit var store: PairStore

    private val notificationPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { refreshChecks() }

    private var checks by mutableStateOf(Checks())
    private var enabled by mutableStateOf(true)

    data class Checks(val usbDebugging: Boolean = false, val notifications: Boolean = false, val battery: Boolean = false, val adb: Boolean = false)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Fully transparent bars: the app background runs behind the status and navigation bars.
        val clear = android.graphics.Color.TRANSPARENT
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.auto(clear, clear),
            navigationBarStyle = SystemBarStyle.auto(clear, clear),
        )
        if (Build.VERSION.SDK_INT >= 29) {
            window.isNavigationBarContrastEnforced = false
            window.isStatusBarContrastEnforced = false
        }
        store = PairStore(this)
        enabled = store.enabled
        if (store.enabled && store.pcs().isNotEmpty()) LinkService.start(this)
        setContent { PairspanTheme { App() } }
    }

    private fun granted(permission: String) = ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED

    private fun refreshChecks() {
        val usb = Settings.Global.getInt(contentResolver, Settings.Global.ADB_ENABLED, 0) == 1
        val notifications = Build.VERSION.SDK_INT < 33 || granted(Manifest.permission.POST_NOTIFICATIONS)
        val battery = getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(packageName)
        checks = checks.copy(usbDebugging = usb, notifications = notifications, battery = battery)
        thread { val adb = LinkService.adbReachable(); runOnUiThread { checks = checks.copy(adb = adb) } }
    }

    private fun toggle(on: Boolean) {
        enabled = on
        store.enabled = on
        if (on) LinkService.start(this) else LinkService.stop(this)
    }

    @SuppressLint("BatteryLife")
    private fun requestBattery() {
        startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")))
    }

    private fun openDeveloperOptions() {
        val dev = Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS)
        startActivity(if (dev.resolveActivity(packageManager) != null) dev else Intent(Settings.ACTION_DEVICE_INFO_SETTINGS))
    }

    // ------------------------------------------------------------------ UI

    @Composable
    private fun App() {
        val state by LinkService.state.collectAsState()
        val lifecycle = LocalLifecycleOwner.current.lifecycle
        DisposableEffect(lifecycle) {
            val obs = LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME) { refreshChecks(); enabled = store.enabled } }
            lifecycle.addObserver(obs)
            onDispose { lifecycle.removeObserver(obs) }
        }
        // Pairing over USB can turn the switch back on from the service.
        LaunchedEffect(state.enabled) { enabled = store.enabled }
        // Wireless ADB comes up right after pairing; look again while the screen is open.
        LaunchedEffect(state.pcs.size, state.pairing) { refreshChecks() }
        Surface(color = MaterialTheme.colorScheme.background, modifier = Modifier.fillMaxSize()) { HomeScreen(state) }
        PairingDialog(state.pairing)
    }

    @Composable
    private fun HomeScreen(state: LinkState) {
        var removing by remember { mutableStateOf<PcView?>(null) }
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = screenPadding(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(vertical = 8.dp)) {
                    Logo(36)
                    Spacer(Modifier.width(12.dp))
                    Text(
                        stringResource(R.string.app_name), modifier = Modifier.weight(1f),
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.ExtraBold, letterSpacing = (-0.9).sp),
                    )
                    Switch(
                        checked = enabled, onCheckedChange = ::toggle,
                        colors = SwitchDefaults.colors(checkedTrackColor = MaterialTheme.colorScheme.primary, checkedThumbColor = MaterialTheme.colorScheme.onPrimary),
                    )
                }
            }
            item { Hero(state) }
            item { SectionLabel(stringResource(R.string.section_setup)) }
            item { Steps(state) }
            if (state.pcs.isNotEmpty()) {
                item { SectionLabel(stringResource(R.string.section_computers)) }
                items(state.pcs.size) { i -> PcCard(state.pcs[i], onRemove = { removing = state.pcs[i] }) }
            }
            item { Footer() }
        }
        removing?.let { pc ->
            AlertDialog(
                onDismissRequest = { removing = null },
                title = { Text(stringResource(R.string.remove_title, pc.name)) },
                text = { Text(stringResource(R.string.remove_body)) },
                confirmButton = { TextButton(onClick = { LinkService.unpair(this, pc.id); removing = null }) { Text(stringResource(R.string.action_remove), color = MaterialTheme.colorScheme.primary) } },
                dismissButton = { TextButton(onClick = { removing = null }) { Text(stringResource(R.string.action_cancel)) } },
            )
        }
    }

    /** System bar and cutout insets plus the screen margin, so content scrolls under transparent bars. */
    @Composable
    private fun screenPadding(): PaddingValues {
        val insets = WindowInsets.safeDrawing.asPaddingValues()
        val dir = androidx.compose.ui.platform.LocalLayoutDirection.current
        return PaddingValues(
            start = insets.calculateStartPadding(dir) + 20.dp,
            end = insets.calculateEndPadding(dir) + 20.dp,
            top = insets.calculateTopPadding() + 12.dp,
            bottom = insets.calculateBottomPadding() + 24.dp,
        )
    }

    @Composable
    private fun Logo(size: Int) {
        Box(
            Modifier.size(size.dp).clip(RoundedCornerShape((size * 0.26f).dp)).background(Brand.Ink),
            contentAlignment = Alignment.Center,
        ) {
            Image(painterResource(R.drawable.ic_pairspan_foreground), null, Modifier.fillMaxSize())
        }
    }

    @Composable
    private fun SectionLabel(text: String) {
        Text(
            text.uppercase(), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 14.dp, start = 4.dp),
        )
    }

    @Composable
    private fun StatusDot(color: Color, halo: Boolean) {
        Box(
            Modifier.size(if (halo) 18.dp else 10.dp).then(if (halo) Modifier.background(color.copy(alpha = .25f), CircleShape) else Modifier),
            contentAlignment = Alignment.Center,
        ) { Box(Modifier.size(10.dp).background(color, CircleShape)) }
    }

    @Composable
    private fun Hero(state: LinkState) {
        val s = LocalStatus.current
        val onlinePc = state.pcs.firstOrNull { it.online }
        val firstPc = state.pcs.firstOrNull()
        val (title, body) = when {
            !enabled -> stringResource(R.string.hero_off_title) to stringResource(R.string.hero_off_body)
            state.pairing == PairingState.Waiting -> stringResource(R.string.hero_pairing_title) to stringResource(R.string.hero_pairing_body)
            firstPc == null -> stringResource(R.string.hero_unpaired_title) to stringResource(R.string.hero_unpaired_body)
            onlinePc != null -> stringResource(R.string.hero_connected_title, onlinePc.name) to stringResource(R.string.hero_connected_body)
            state.net == NetState.OFFLINE -> stringResource(R.string.hero_offline_title) to stringResource(R.string.hero_offline_body)
            else -> stringResource(R.string.hero_waiting_title, firstPc.name) to stringResource(R.string.hero_waiting_body)
        }
        val (chip, chipColor) = when {
            !enabled -> stringResource(R.string.status_off) to s.heroMuted
            onlinePc != null -> stringResource(R.string.status_online) to s.online
            firstPc == null -> stringResource(R.string.status_setup) to s.action
            state.net == NetState.ONLINE -> stringResource(R.string.status_waiting) to s.action
            state.net == NetState.CONNECTING -> stringResource(R.string.status_connecting) to s.heroMuted
            else -> stringResource(R.string.status_offline) to s.heroMuted
        }
        Column(Modifier.fillMaxWidth().clip(MaterialTheme.shapes.large).background(s.hero).padding(24.dp)) {
            Row(
                Modifier.clip(CircleShape).background(s.onHero.copy(alpha = .08f)).padding(horizontal = 12.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                StatusDot(chipColor, halo = enabled && onlinePc != null)
                Spacer(Modifier.width(8.dp))
                Text(chip, color = s.onHero, style = MaterialTheme.typography.labelLarge)
                if (enabled && state.activeStreams > 0) {
                    Text("  ·  " + stringResource(R.string.streams_active), color = s.heroMuted, style = MaterialTheme.typography.bodySmall)
                }
            }
            Spacer(Modifier.height(18.dp))
            Text(title, style = MaterialTheme.typography.headlineMedium, color = s.onHero)
            Spacer(Modifier.height(8.dp))
            Text(body, style = MaterialTheme.typography.bodyMedium, color = s.heroMuted)
        }
    }

    /** Every setup step with its state, in order, so it is clear what is done and what comes next. */
    @Composable
    private fun Steps(state: LinkState) {
        val paired = state.pcs.isNotEmpty()
        val pcName = state.pcs.firstOrNull()?.name.orEmpty()
        Group {
            StepRow(1, stringResource(R.string.step_usb), stringResource(if (checks.usbDebugging) R.string.step_usb_ok else R.string.step_usb_todo),
                checks.usbDebugging, stringResource(R.string.action_open), ::openDeveloperOptions)
            Divider()
            StepRow(2, stringResource(R.string.step_pair),
                when {
                    paired -> stringResource(R.string.step_pair_ok, pcName)
                    state.pairing == PairingState.Waiting -> stringResource(R.string.step_pair_now)
                    else -> stringResource(R.string.step_pair_todo)
                },
                paired, null, null)
            Divider()
            StepRow(3, stringResource(R.string.step_adb), stringResource(if (checks.adb) R.string.step_adb_ok else R.string.step_adb_todo),
                checks.adb, stringResource(R.string.action_check)) { refreshChecks() }
            Divider()
            StepRow(4, stringResource(R.string.step_notifications), stringResource(R.string.step_notifications_d), checks.notifications, stringResource(R.string.action_allow)) {
                if (Build.VERSION.SDK_INT >= 33) notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
            Divider()
            StepRow(5, stringResource(R.string.step_battery), stringResource(R.string.step_battery_d), checks.battery, stringResource(R.string.action_allow), ::requestBattery)
        }
    }

    @Composable
    private fun Divider() = HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)

    @Composable
    private fun StepRow(n: Int, title: String, detail: String, done: Boolean, action: String?, onAction: (() -> Unit)?) {
        val s = LocalStatus.current
        Row(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.size(28.dp).background(if (done) s.online.copy(alpha = .18f) else s.action.copy(alpha = .14f), CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                if (done) Icon(Icons.Filled.Check, null, tint = s.online, modifier = Modifier.size(16.dp))
                else Text(n.toString(), color = s.action, style = MaterialTheme.typography.labelLarge)
            }
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (!done && action != null && onAction != null) {
                Spacer(Modifier.width(8.dp))
                Button(
                    onClick = onAction, contentPadding = PaddingValues(horizontal = 16.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.onBackground, contentColor = MaterialTheme.colorScheme.background),
                ) { Text(action, style = MaterialTheme.typography.labelLarge) }
            }
        }
    }

    @Composable
    private fun PcCard(pc: PcView, onRemove: () -> Unit) {
        val s = LocalStatus.current
        val online = enabled && pc.online
        Row(
            Modifier.fillMaxWidth().clip(MaterialTheme.shapes.large).background(MaterialTheme.colorScheme.surface)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, MaterialTheme.shapes.large).padding(start = 18.dp, end = 6.dp, top = 14.dp, bottom = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            StatusDot(if (online) s.online else s.offline, halo = online)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(pc.name, style = MaterialTheme.typography.titleMedium)
                val os = when (pc.os) { "macos" -> "macOS"; "linux" -> "Linux"; "windows" -> "Windows"; else -> "" }
                Text(
                    listOf(os, stringResource(if (online) R.string.pc_online else R.string.pc_offline)).filter { it.isNotEmpty() }.joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            TextButton(onClick = onRemove) { Text(stringResource(R.string.action_remove), color = MaterialTheme.colorScheme.primary) }
        }
    }

    @Composable
    private fun Footer() {
        var update by remember { mutableStateOf<Pair<UpdateStatus, String>>(UpdateStatus.IDLE to "") }
        var release by remember { mutableStateOf<UpdateReleaseInfo?>(null) }
        var progress by remember { mutableIntStateOf(0) }
        val text = when (update.first) {
            UpdateStatus.CHECKING -> stringResource(R.string.update_checking)
            UpdateStatus.UP_TO_DATE -> stringResource(R.string.update_latest, BuildConfig.VERSION_NAME)
            UpdateStatus.UPDATE_AVAILABLE -> stringResource(R.string.update_available, release?.versionName.orEmpty())
            UpdateStatus.DOWNLOADING -> stringResource(R.string.update_downloading, progress)
            UpdateStatus.READY_TO_INSTALL -> stringResource(R.string.update_install)
            UpdateStatus.ERROR -> stringResource(R.string.update_error, update.second)
            else -> stringResource(R.string.settings_version, BuildConfig.VERSION_NAME)
        }
        Column(Modifier.fillMaxWidth().padding(top = 16.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(text, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
            when (update.first) {
                UpdateStatus.CHECKING, UpdateStatus.DOWNLOADING -> CircularProgressIndicator(Modifier.padding(top = 8.dp).size(20.dp), strokeWidth = 2.dp)
                UpdateStatus.UPDATE_AVAILABLE -> TextButton(onClick = {
                    val info = release ?: return@TextButton
                    update = UpdateStatus.DOWNLOADING to ""
                    OtaUpdateManager.downloadApk(this@MainActivity, info,
                        onProgress = { p -> runOnUiThread { progress = p } },
                        onComplete = { f -> runOnUiThread { update = UpdateStatus.READY_TO_INSTALL to ""; OtaUpdateManager.triggerInstall(this@MainActivity, f) } },
                        onError = { e -> runOnUiThread { update = UpdateStatus.ERROR to e } })
                }) { Text(stringResource(R.string.update_download), color = MaterialTheme.colorScheme.primary) }
                else -> TextButton(onClick = {
                    update = UpdateStatus.CHECKING to ""
                    OtaUpdateManager.checkForUpdates { st, info, err -> runOnUiThread { release = info; update = st to (err ?: "") } }
                }) { Text(stringResource(R.string.update_check), color = MaterialTheme.colorScheme.primary) }
            }
        }
    }

    @Composable
    private fun PairingDialog(pairing: PairingState) {
        when (pairing) {
            PairingState.Idle, PairingState.Waiting -> {}
            is PairingState.Done -> AlertDialog(
                onDismissRequest = LinkService::clearPairing,
                icon = { Icon(Icons.Filled.Check, null, tint = LocalStatus.current.online) },
                title = { Text(stringResource(R.string.pair_done, pairing.pcName)) },
                text = { Text(stringResource(R.string.pair_done_d)) },
                confirmButton = { Button(onClick = LinkService::clearPairing) { Text(stringResource(R.string.action_ok)) } },
            )
            is PairingState.Failed -> AlertDialog(
                onDismissRequest = LinkService::clearPairing,
                title = { Text(stringResource(R.string.pair_failed)) },
                text = { Text(pairing.reason) },
                confirmButton = { Button(onClick = LinkService::clearPairing) { Text(stringResource(R.string.action_ok)) } },
            )
        }
    }

    @Composable
    private fun Group(content: @Composable ColumnScope.() -> Unit) {
        Column(
            Modifier.fillMaxWidth().clip(MaterialTheme.shapes.large).background(MaterialTheme.colorScheme.surface)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, MaterialTheme.shapes.large),
            content = content,
        )
    }
}
