package com.pairspan.app.ui

import android.Manifest
import android.annotation.SuppressLint
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import com.pairspan.app.BuildConfig
import com.pairspan.app.OtaUpdateManager
import com.pairspan.app.PairspanCaptureActivity
import com.pairspan.app.R
import com.pairspan.app.UpdateReleaseInfo
import com.pairspan.app.UpdateStatus
import com.pairspan.app.core.Pairing
import com.pairspan.app.link.GatewayState
import com.pairspan.app.link.LinkService
import com.pairspan.app.link.LinkState
import com.pairspan.app.link.PairStore
import com.pairspan.app.link.PairingState
import com.pairspan.app.link.PcView
import kotlin.concurrent.thread

class MainActivity : ComponentActivity() {
    private lateinit var store: PairStore

    private val scanner = registerForActivityResult(ScanContract()) { result ->
        result.contents?.let(::handlePairInput)
    }
    private val notificationPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { refreshChecks() }
    private val cameraPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { refreshChecks() }

    private var checks by mutableStateOf(Checks())

    data class Checks(val notifications: Boolean = false, val battery: Boolean = false, val adb: Boolean = false, val camera: Boolean = false)

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
        if (store.enabled && store.pcs().isNotEmpty()) LinkService.start(this)
        intent?.dataString?.let(::handlePairInput)
        setContent { PairspanTheme { App() } }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.dataString?.let(::handlePairInput)
    }

    /** Accepts `pairspan://pair?g=…&t=…&k=…` links/QR codes or a typed 20-character code. */
    private fun handlePairInput(raw: String) {
        val trimmed = raw.trim()
        if (trimmed.startsWith("pairspan://")) {
            val uri = Uri.parse(trimmed)
            if (uri.host != "pair") return
            val secret = Pairing.normalize(uri.getQueryParameter("t").orEmpty())
            val gateway = uri.getQueryParameter("g")?.takeIf { it.startsWith("wss://") || it.startsWith("ws://") }
            val pc = uri.getQueryParameter("k")?.takeIf { it.matches(Regex("[0-9a-f]{32}")) }
            LinkService.pair(this, secret, gateway, pc)
        } else {
            LinkService.pair(this, Pairing.normalize(trimmed), null, null)
        }
    }

    private fun scan() {
        scanner.launch(
            ScanOptions().setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                .setCaptureActivity(PairspanCaptureActivity::class.java)
                .setOrientationLocked(false).setBeepEnabled(false)
                .setPrompt(getString(R.string.scan_prompt)),
        )
    }

    private fun granted(permission: String) = ContextCompat.checkSelfPermission(this, permission) == PackageManager.PERMISSION_GRANTED

    private fun refreshChecks() {
        val notifications = Build.VERSION.SDK_INT < 33 || granted(Manifest.permission.POST_NOTIFICATIONS)
        val battery = getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(packageName)
        val camera = granted(Manifest.permission.CAMERA)
        checks = checks.copy(notifications = notifications, battery = battery, camera = camera)
        thread { val adb = LinkService.adbReachable(); runOnUiThread { checks = checks.copy(adb = adb) } }
    }

    @SuppressLint("BatteryLife")
    private fun requestBattery() {
        startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")))
    }

    private fun copy(text: String) {
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("Pairspan", text))
        Toast.makeText(this, R.string.copied, Toast.LENGTH_SHORT).show()
    }

    // ------------------------------------------------------------------ UI

    @Composable
    private fun App() {
        val state by LinkService.state.collectAsState()
        var screen by rememberSaveable { mutableStateOf("home") }
        val lifecycle = LocalLifecycleOwner.current.lifecycle
        DisposableEffect(lifecycle) {
            val obs = LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME) refreshChecks() }
            lifecycle.addObserver(obs)
            onDispose { lifecycle.removeObserver(obs) }
        }
        Surface(color = MaterialTheme.colorScheme.background, modifier = Modifier.fillMaxSize()) {
            when (screen) {
                "settings" -> {
                    BackHandler { screen = "home" }
                    SettingsScreen(onBack = { screen = "home" })
                }
                else -> HomeScreen(state, onSettings = { screen = "settings" })
            }
        }
        PairingDialog(state.pairing)
    }

    @Composable
    private fun HomeScreen(state: LinkState, onSettings: () -> Unit) {
        val paired = state.pcs.isNotEmpty()
        var enterCode by remember { mutableStateOf(false) }
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
                        style = MaterialTheme.typography.titleLarge.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.ExtraBold, letterSpacing = (-0.9).sp),
                    )
                    IconButton(onClick = onSettings) { Icon(Icons.Filled.Settings, stringResource(R.string.settings_title)) }
                }
            }
            item { Hero(state, onScan = ::scan, onEnterCode = { enterCode = true }) }
            if (paired) {
                item { SectionLabel(stringResource(R.string.section_computers)) }
                items(state.pcs.size) { i -> PcCard(state.pcs[i], onRemove = { removing = state.pcs[i] }) }
                item {
                    OutlinedButton(
                        onClick = ::scan, modifier = Modifier.fillMaxWidth().height(50.dp),
                        border = androidx.compose.foundation.BorderStroke(1.5.dp, MaterialTheme.colorScheme.onBackground),
                        colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.onBackground),
                    ) { Text(stringResource(R.string.action_pair_another)) }
                }
            }
            // Only the steps still to do; the section disappears once the phone is fully set up.
            if (!(checks.adb && checks.notifications && checks.battery && checks.camera)) {
                item { SectionLabel(stringResource(R.string.section_setup)) }
                item { SetupCard() }
            }
            item {
                Text(
                    stringResource(R.string.settings_version, BuildConfig.VERSION_NAME),
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.fillMaxWidth().padding(vertical = 16.dp), textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                )
            }
        }
        if (enterCode) CodeDialog(onDismiss = { enterCode = false }, onPair = { enterCode = false; handlePairInput(it) })
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
    private fun Hero(state: LinkState, onScan: () -> Unit, onEnterCode: () -> Unit) {
        val s = LocalStatus.current
        val onlinePc = state.pcs.firstOrNull { it.online }
        val firstPc = state.pcs.firstOrNull()
        val (title, body) = when {
            firstPc == null -> stringResource(R.string.hero_unpaired_title) to stringResource(R.string.hero_unpaired_body)
            onlinePc != null -> stringResource(R.string.hero_connected_title, onlinePc.name) to stringResource(R.string.hero_connected_body)
            state.gateway == GatewayState.OFFLINE -> stringResource(R.string.hero_offline_title) to stringResource(R.string.hero_offline_body)
            else -> stringResource(R.string.hero_waiting_title, firstPc.name) to stringResource(R.string.hero_waiting_body)
        }
        val (chip, chipColor) = when {
            onlinePc != null -> stringResource(R.string.status_online) to s.online
            firstPc == null -> stringResource(R.string.status_ready) to s.action
            state.gateway == GatewayState.CONNECTING -> stringResource(R.string.status_connecting) to s.heroMuted
            else -> stringResource(R.string.status_offline) to s.heroMuted
        }
        Column(
            Modifier.fillMaxWidth().clip(MaterialTheme.shapes.large).background(s.hero).padding(24.dp),
        ) {
            Row(
                Modifier.clip(CircleShape).background(s.onHero.copy(alpha = .08f)).padding(horizontal = 12.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                StatusDot(chipColor, halo = onlinePc != null)
                Spacer(Modifier.width(8.dp))
                Text(chip, color = s.onHero, style = MaterialTheme.typography.labelLarge)
                if (state.activeStreams > 0) {
                    Text("  ·  " + stringResource(R.string.streams_active), color = s.heroMuted, style = MaterialTheme.typography.bodySmall)
                }
            }
            Spacer(Modifier.height(18.dp))
            Text(title, style = MaterialTheme.typography.headlineMedium, color = s.onHero)
            Spacer(Modifier.height(8.dp))
            Text(body, style = MaterialTheme.typography.bodyMedium, color = s.heroMuted)
            if (firstPc == null) {
                Spacer(Modifier.height(22.dp))
                Button(
                    onClick = onScan, modifier = Modifier.fillMaxWidth().height(52.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
                ) { Text(stringResource(R.string.action_scan)) }
                Spacer(Modifier.height(8.dp))
                OutlinedButton(
                    onClick = onEnterCode, modifier = Modifier.fillMaxWidth().height(52.dp),
                    border = androidx.compose.foundation.BorderStroke(1.dp, s.onHero.copy(alpha = .25f)),
                    colors = ButtonDefaults.outlinedButtonColors(contentColor = s.onHero),
                ) { Text(stringResource(R.string.action_enter_code)) }
            }
        }
    }

    @Composable
    private fun PcCard(pc: PcView, onRemove: () -> Unit) {
        val s = LocalStatus.current
        Row(
            Modifier.fillMaxWidth().clip(MaterialTheme.shapes.large).background(MaterialTheme.colorScheme.surface)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, MaterialTheme.shapes.large).padding(start = 18.dp, end = 6.dp, top = 14.dp, bottom = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            StatusDot(if (pc.online) s.online else s.offline, halo = pc.online)
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(pc.name, style = MaterialTheme.typography.titleMedium)
                val os = when (pc.os) { "macos" -> "macOS"; "linux" -> "Linux"; "windows" -> "Windows"; else -> "" }
                Text(
                    listOf(os, stringResource(if (pc.online) R.string.pc_online else R.string.pc_offline)).filter { it.isNotEmpty() }.joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            TextButton(onClick = onRemove) { Text(stringResource(R.string.action_remove), color = MaterialTheme.colorScheme.primary) }
        }
    }

    @Composable
    private fun SetupCard() {
        Column(
            Modifier.fillMaxWidth().clip(MaterialTheme.shapes.large).background(MaterialTheme.colorScheme.surface)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, MaterialTheme.shapes.large),
        ) {
            val rows = buildList<@Composable () -> Unit> {
                if (!checks.adb) add { SetupRow(stringResource(R.string.setup_adb), stringResource(R.string.setup_adb_missing), false, stringResource(R.string.action_check)) { refreshChecks() } }
                if (!checks.notifications) add {
                    SetupRow(stringResource(R.string.setup_notifications), stringResource(R.string.setup_notifications_d), false, stringResource(R.string.action_allow)) {
                        if (Build.VERSION.SDK_INT >= 33) notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }
                }
                if (!checks.battery) add { SetupRow(stringResource(R.string.setup_battery), stringResource(R.string.setup_battery_d), false, stringResource(R.string.action_allow), ::requestBattery) }
                if (!checks.camera) add {
                    SetupRow(stringResource(R.string.setup_camera), stringResource(R.string.setup_camera_d), false, stringResource(R.string.action_allow)) {
                        cameraPermission.launch(Manifest.permission.CAMERA)
                    }
                }
            }
            rows.forEachIndexed { i, row ->
                if (i > 0) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                row()
            }
        }
    }

    @Composable
    private fun SetupRow(title: String, detail: String, done: Boolean, action: String, onAction: () -> Unit) {
        val s = LocalStatus.current
        Row(Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.size(26.dp).background(if (done) s.online.copy(alpha = .18f) else s.action.copy(alpha = .14f), CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                if (done) Icon(Icons.Filled.Check, null, tint = s.online, modifier = Modifier.size(16.dp))
                else Box(Modifier.size(8.dp).background(s.action, CircleShape))
            }
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if (!done) {
                Spacer(Modifier.width(8.dp))
                Button(
                    onClick = onAction, contentPadding = PaddingValues(horizontal = 16.dp),
                    colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.onBackground, contentColor = MaterialTheme.colorScheme.background),
                ) { Text(action, style = MaterialTheme.typography.labelLarge) }
            }
        }
    }

    @Composable
    private fun CodeDialog(onDismiss: () -> Unit, onPair: (String) -> Unit) {
        var value by remember { mutableStateOf(TextFieldValue("")) }
        val clean = Pairing.normalize(value.text)
        AlertDialog(
            onDismissRequest = onDismiss,
            title = { Text(stringResource(R.string.code_title)) },
            text = {
                OutlinedTextField(
                    value = value,
                    onValueChange = { v ->
                        val formatted = Pairing.normalize(v.text).take(20).chunked(4).joinToString("-")
                        value = TextFieldValue(formatted, TextRange(formatted.length))
                    },
                    placeholder = { Text(stringResource(R.string.code_hint), fontFamily = FontFamily.Monospace) },
                    textStyle = MaterialTheme.typography.bodyLarge.copy(fontFamily = FontFamily.Monospace),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Characters, autoCorrectEnabled = false),
                    shape = MaterialTheme.shapes.small,
                )
            },
            confirmButton = { Button(onClick = { onPair(clean) }, enabled = Pairing.isValid(clean)) { Text(stringResource(R.string.action_pair)) } },
            dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) } },
        )
    }

    @Composable
    private fun PairingDialog(pairing: PairingState) {
        when (pairing) {
            PairingState.Idle -> {}
            PairingState.Waiting -> AlertDialog(
                onDismissRequest = {},
                icon = { CircularProgressIndicator(color = MaterialTheme.colorScheme.primary, strokeWidth = 3.dp, modifier = Modifier.size(28.dp)) },
                title = { Text(stringResource(R.string.pair_waiting)) },
                text = { Text(stringResource(R.string.pair_waiting_d)) },
                confirmButton = {},
            )
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
                confirmButton = { Button(onClick = { LinkService.clearPairing(); scan() }) { Text(stringResource(R.string.action_try_again)) } },
                dismissButton = { TextButton(onClick = LinkService::clearPairing) { Text(stringResource(R.string.action_cancel)) } },
            )
        }
    }

    @Composable
    private fun SettingsScreen(onBack: () -> Unit) {
        var stay by remember { mutableStateOf(store.enabled) }
        var boot by remember { mutableStateOf(store.startOnBoot) }
        var gateway by remember { mutableStateOf(store.gatewayUrl) }
        var update by remember { mutableStateOf<Pair<UpdateStatus, String>>(UpdateStatus.IDLE to "") }
        var release by remember { mutableStateOf<UpdateReleaseInfo?>(null) }
        var progress by remember { mutableIntStateOf(0) }

        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            contentPadding = screenPadding(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) }
                    Text(stringResource(R.string.settings_title), style = MaterialTheme.typography.headlineSmall)
                }
            }
            item { SectionLabel(stringResource(R.string.settings_connection)) }
            item {
                Group {
                    SwitchRow(stringResource(R.string.settings_stay), stringResource(R.string.settings_stay_d), stay) {
                        stay = it; store.enabled = it
                        if (it) LinkService.start(this@MainActivity) else LinkService.stop(this@MainActivity)
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                    SwitchRow(stringResource(R.string.settings_boot), stringResource(R.string.settings_boot_d), boot) { boot = it; store.startOnBoot = it }
                }
            }
            item { SectionLabel(stringResource(R.string.settings_gateway)) }
            item {
                Group {
                    Column(Modifier.padding(16.dp)) {
                        OutlinedTextField(
                            value = gateway, onValueChange = { gateway = it.trim() }, singleLine = true,
                            textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                            modifier = Modifier.fillMaxWidth(), shape = MaterialTheme.shapes.small,
                        )
                        Spacer(Modifier.height(10.dp))
                        Button(
                            onClick = { store.gatewayUrl = gateway; LinkService.reconnect(this@MainActivity) },
                            enabled = (gateway.startsWith("wss://") || gateway.startsWith("ws://")) && gateway != store.gatewayUrl,
                        ) { Text(stringResource(R.string.action_save)) }
                    }
                }
            }
            item { SectionLabel(stringResource(R.string.settings_device)) }
            item {
                Group {
                    Column(Modifier.fillMaxWidth().clickable { copy(store.deviceId) }.padding(16.dp)) {
                        Text(stringResource(R.string.settings_device_id), style = MaterialTheme.typography.titleMedium)
                        Text(store.deviceId, style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
            item { SectionLabel(stringResource(R.string.settings_updates)) }
            item {
                Group {
                    Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                        val text = when (update.first) {
                            UpdateStatus.CHECKING -> stringResource(R.string.update_checking)
                            UpdateStatus.UP_TO_DATE -> stringResource(R.string.update_latest, BuildConfig.VERSION_NAME)
                            UpdateStatus.UPDATE_AVAILABLE -> stringResource(R.string.update_available, release?.versionName.orEmpty())
                            UpdateStatus.DOWNLOADING -> stringResource(R.string.update_downloading, progress)
                            UpdateStatus.READY_TO_INSTALL -> stringResource(R.string.update_install)
                            UpdateStatus.ERROR -> stringResource(R.string.update_error, update.second)
                            else -> stringResource(R.string.update_check)
                        }
                        Text(text, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                        when (update.first) {
                            UpdateStatus.UPDATE_AVAILABLE -> Button(onClick = {
                                val info = release ?: return@Button
                                update = UpdateStatus.DOWNLOADING to ""
                                OtaUpdateManager.downloadApk(this@MainActivity, info,
                                    onProgress = { p -> runOnUiThread { progress = p } },
                                    onComplete = { f -> runOnUiThread { update = UpdateStatus.READY_TO_INSTALL to ""; OtaUpdateManager.triggerInstall(this@MainActivity, f) } },
                                    onError = { e -> runOnUiThread { update = UpdateStatus.ERROR to e } })
                            }) { Text(stringResource(R.string.update_download)) }
                            UpdateStatus.CHECKING, UpdateStatus.DOWNLOADING -> CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp)
                            else -> FilledTonalButton(
                                onClick = {
                                    update = UpdateStatus.CHECKING to ""
                                    OtaUpdateManager.checkForUpdates { st, info, err -> runOnUiThread { release = info; update = st to (err ?: "") } }
                                },
                                colors = ButtonDefaults.filledTonalButtonColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
                            ) { Text(stringResource(R.string.update_check)) }
                        }
                    }
                }
            }
            item { SectionLabel(stringResource(R.string.settings_about)) }
            item {
                Group {
                    Column(Modifier.padding(16.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Logo(28)
                            Spacer(Modifier.width(10.dp))
                            Text(stringResource(R.string.settings_version, BuildConfig.VERSION_NAME), style = MaterialTheme.typography.titleMedium)
                        }
                        Spacer(Modifier.height(8.dp))
                        Text(stringResource(R.string.licenses_text), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
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

    @Composable
    private fun SwitchRow(title: String, detail: String, checked: Boolean, onChange: (Boolean) -> Unit) {
        Row(Modifier.fillMaxWidth().clickable { onChange(!checked) }.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                Text(detail, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Switch(
                checked = checked, onCheckedChange = onChange,
                colors = SwitchDefaults.colors(checkedTrackColor = MaterialTheme.colorScheme.primary, checkedThumbColor = MaterialTheme.colorScheme.onPrimary),
            )
        }
    }
}
