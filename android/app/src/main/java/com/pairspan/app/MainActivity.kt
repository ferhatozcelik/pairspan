package com.pairspan.app

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Intent
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.compose.BackHandler
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Settings
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.Image
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.unit.dp
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.delay

class MainActivity : ComponentActivity() {
    private enum class Screen { Home, Settings, Licenses }

    private val prefs by lazy { getSharedPreferences("gateway", Context.MODE_PRIVATE) }
    private var notice by mutableStateOf("")
    private var code by mutableStateOf("")
    private var gatewayState by mutableStateOf(0)
    private var status by mutableStateOf("")
    private var adbReady by mutableStateOf(false)
    private var hasSession by mutableStateOf(false)
    private var adminActive by mutableStateOf(false)
    private var notificationGranted by mutableStateOf(false)
    private var cameraGranted by mutableStateOf(false)
    private var adbDetail by mutableStateOf("")
    private var screen by mutableStateOf(Screen.Home)
    private var keepScreenOn by mutableStateOf(false)
    private var startOnBoot by mutableStateOf(true)
    private var enabled by mutableStateOf(true)

    private var secureGranted by mutableStateOf(false)
    private var servicesEnabled by mutableStateOf(false)
    private var adbWaitTicks by mutableStateOf(0)
    private var askedNotification = false
    private var askedCamera = false
    private var appliedSystemAccess = false

    private val adminResult = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { refreshPermissions() }
    private val notificationPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { refreshPermissions() }
    private val cameraPermission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { refreshPermissions() }
    private val scanner = registerForActivityResult(ScanContract()) { result ->
        if (result.contents != null) receivePairLink(result.contents)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        ShellBridge.init(this)
        prefs.edit().remove("gatewayToken").remove("gatewaySecret").remove("keepScreen").remove("pinHash").remove("pinSalt").apply()
        keepScreenOn = prefs.getBoolean("keepScreenOn", false)
        startOnBoot = prefs.getBoolean("startOnBoot", true)
        enabled = prefs.getBoolean("enabled", true)
        applyKeepScreen()
        refreshPermissions()
        handleIntent(intent)
        setContent { PairspanUi() }
        resumeSession()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIntent(intent)
    }

    private fun applyKeepScreen() {
        if (keepScreenOn) window.addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        else window.clearFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }

    private fun shareApp() {
        val send = Intent(Intent.ACTION_SEND).setType("text/plain")
            .putExtra(Intent.EXTRA_TEXT, getString(R.string.share_message))
        startActivity(Intent.createChooser(send, getString(R.string.settings_share)))
    }

    private fun refreshPermissions() {
        adminActive = PairspanDeviceAdminReceiver.isActive(this)
        notificationGranted = Build.VERSION.SDK_INT < 33 || checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED
        cameraGranted = checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
        secureGranted = checkSelfPermission(Manifest.permission.WRITE_SECURE_SETTINGS) == PackageManager.PERMISSION_GRANTED
        servicesEnabled = listed("enabled_accessibility_services", PairspanAccessibilityService::class.java)
            && listed("enabled_notification_listeners", PairspanNotificationListener::class.java)
    }

    private fun listed(key: String, service: Class<*>): Boolean {
        val name = ComponentName(this, service).flattenToString()
        return (Settings.Secure.getString(contentResolver, key) ?: "").split(':').any { it == name }
    }

    /** With WRITE_SECURE_SETTINGS the app turns on its own services and ADB without sending the user to Settings. */
    private fun applySystemAccess() {
        if (!secureGranted) return
        try {
            fun append(key: String, service: Class<*>) {
                val name = ComponentName(this, service).flattenToString()
                val current = Settings.Secure.getString(contentResolver, key) ?: ""
                if (current.split(':').none { it == name }) {
                    Settings.Secure.putString(contentResolver, key, if (current.isEmpty()) name else "$current:$name")
                }
            }
            append("enabled_accessibility_services", PairspanAccessibilityService::class.java)
            Settings.Secure.putInt(contentResolver, "accessibility_enabled", 1)
            append("enabled_notification_listeners", PairspanNotificationListener::class.java)
            Settings.Global.putInt(contentResolver, Settings.Global.ADB_ENABLED, 1)
            if (Build.VERSION.SDK_INT >= 30) Settings.Global.putInt(contentResolver, "adb_wifi_enabled", 1)
        } catch (_: Exception) { }
        refreshPermissions()
    }

    private fun openAppSettings() {
        startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, android.net.Uri.parse("package:$packageName")))
    }

    private fun requestNotification() {
        if (Build.VERSION.SDK_INT < 33) return
        if (askedNotification && !shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS)) openAppSettings()
        else { askedNotification = true; notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS) }
    }

    private fun requestCamera() {
        if (askedCamera && !shouldShowRequestPermissionRationale(Manifest.permission.CAMERA)) openAppSettings()
        else { askedCamera = true; cameraPermission.launch(Manifest.permission.CAMERA) }
    }

    private fun copyText(label: String, value: String) {
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText(label, value))
        notice = getString(R.string.notice_copied)
    }

    private fun handleIntent(intent: Intent?) {
        if (intent == null) return
        intent.dataString?.let(::receivePairLink)
    }

    private fun receivePairLink(raw: String) {
        try {
            val uri = android.net.Uri.parse(raw)
            if (uri.scheme != "pairspan" || uri.host != "pair") return
            val incoming = uri.getQueryParameter("code")?.uppercase()?.filter { it.isLetterOrDigit() } ?: return
            val gateway = uri.getQueryParameter("g") ?: return
            val base = normalizeBaseUrl(gateway)
            if (!incoming.matches(Regex("[2-9A-HJKMNP-Z]{20}")) || base == null) {
                notice = getString(R.string.notice_qr_invalid); return
            }
            prefs.edit().putString("gatewayUrl", base).putLong("gatewayUrlAt", System.currentTimeMillis()).apply()
            code = incoming
            pair(incoming, fromQr = true)
        } catch (_: Exception) { notice = getString(R.string.notice_qr_unreadable) }
    }

    private fun normalizeBaseUrl(raw: String): String? {
        if (raw.isBlank()) return ""
        return try {
            val uri = android.net.Uri.parse(raw.trim())
            val scheme = uri.scheme?.lowercase() ?: return null
            val path = uri.path ?: ""
            if (scheme !in setOf("http", "https", "ws", "wss") || uri.host.isNullOrBlank()
                || uri.encodedAuthority?.contains('@') == true || uri.query != null || uri.fragment != null
                || path !in setOf("", "/", "/ws")) return null
            "$scheme://${uri.authority}"
        } catch (_: Exception) { null }
    }
    private fun resumeSession() {
        hasSession = prefs.getString("sessionId", "")?.matches(Regex("[0-9a-f]{48}")) == true
        if (hasSession && enabled) startGateway(null)
    }

    /** Master switch: off stops the connection service but keeps the pairing, on reconnects with it. */
    private fun switchEnabled(value: Boolean) {
        enabled = value
        prefs.edit().putBoolean("enabled", value).apply()
        if (value) { if (hasSession) startGateway(null) }
        else { stopService(Intent(this, GatewayService::class.java)); GatewayService.status = "" }
    }
    private fun pair(value: String, fromQr: Boolean = false) {
        if (!value.matches(Regex("[2-9A-HJKMNP-Z]{20}"))) { notice = getString(R.string.notice_enter_code); return }
        if (!fromQr) prefs.edit().remove("gatewayUrl").apply()
        beginPair(value)
    }
    private fun beginPair(value: String) {
        if (!enabled) switchEnabled(true)
        GatewayService.pairError = ""
        prefs.edit().remove("sessionId").apply()
        hasSession = false
        code = ""
        startGateway(value)
        notice = getString(R.string.notice_pairing)
    }
    private fun startGateway(pairCode: String?, direct: Boolean = false) {
        val service = Intent(this, GatewayService::class.java)
        if (pairCode != null) service.putExtra("pairCode", pairCode)
        if (direct) service.putExtra("direct", true)
        startForegroundService(service)
        ShellBridge.reconnect()
    }

    private fun disconnect() {
        stopService(Intent(this, GatewayService::class.java))
        prefs.edit().remove("sessionId").remove("gatewayUrl").remove("gatewayUrlAt").remove("clientId").apply()
        hasSession = false
        notice = getString(R.string.notice_removed)
    }
    @Composable private fun PairspanUi() {
        LaunchedEffect(Unit) {
            var retry = 0
            var probeTick = 0
            while (true) {
                if (probeTick++ % 10 == 0) GatewayProbe.check(this@MainActivity) { state -> runOnUiThread { gatewayState = state } }
                status = GatewayService.status
                if (!notificationGranted && !askedNotification && Build.VERSION.SDK_INT >= 33) requestNotification()
                else if (notificationGranted && !cameraGranted && !askedCamera) requestCamera()
                if (secureGranted && !appliedSystemAccess) { appliedSystemAccess = true; applySystemAccess() }
                adbReady = ShellBridge.ready()
                adbWaitTicks = if (adbReady) 0 else adbWaitTicks + 1
                adbDetail = ShellBridge.detail()
                if (!adbReady && retry++ % 8 == 0) ShellBridge.reconnect()
                hasSession = prefs.getString("sessionId", "")?.matches(Regex("[0-9a-f]{48}")) == true
                if (GatewayService.pairError.isNotEmpty() || (hasSession && notice == getString(R.string.notice_pairing))) notice = ""
                refreshPermissions()
                delay(1000)
            }
        }
        MaterialTheme(colorScheme = lightColorScheme(primary = Palette.Primary, background = Palette.Background)) {
            MainScreen()
        }
    }

    @Composable private fun MainScreen() {
        BackHandler(enabled = screen != Screen.Home) {
            screen = if (screen == Screen.Licenses) Screen.Settings else Screen.Home
        }
        Box(Modifier.fillMaxSize().background(Palette.Background)) {
            when (screen) {
                Screen.Home -> HomeContent()
                Screen.Settings -> SettingsContent()
                Screen.Licenses -> LicensesContent()
            }
        }
    }

    @Composable private fun HomeContent() {
        Column(Modifier.fillMaxSize().safeDrawingPadding().imePadding().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Image(painterResource(R.drawable.ic_pairspan_logo), contentDescription = null, modifier = Modifier.size(42.dp))
                Spacer(Modifier.width(12.dp))
                Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
                Box(Modifier.clickable { switchEnabled(!enabled) }.padding(8.dp)) { PillSwitch(enabled) }
                IconButton(onClick = { screen = Screen.Settings }) {
                    Icon(Icons.Filled.Settings, contentDescription = stringResource(R.string.settings_title), tint = Palette.Title)
                }
            }
            Text(stringResource(R.string.tagline), style = MaterialTheme.typography.bodySmall, color = Palette.Gray, modifier = Modifier.padding(top = 4.dp))
            Spacer(Modifier.height(20.dp))
            HomeScreen()
            if (notice.isNotEmpty()) { Spacer(Modifier.height(12.dp)); Text(notice, color = Palette.Primary) }
        }
    }

    @Composable private fun SettingsContent() {
        Column(Modifier.fillMaxSize().safeDrawingPadding()) {
            SubTopBar(stringResource(R.string.settings_title)) { screen = Screen.Home }
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).padding(bottom = 28.dp)) {
                SectionTitle(stringResource(R.string.settings_section_preferences))
                GroupCard {
                    SwitchRow(stringResource(R.string.settings_keep_screen), stringResource(R.string.settings_keep_screen_d), keepScreenOn, true) {
                        keepScreenOn = it; prefs.edit().putBoolean("keepScreenOn", it).apply(); applyKeepScreen()
                    }
                    SwitchRow(stringResource(R.string.step_admin), stringResource(R.string.step_admin_d), adminActive, true) {
                        if (it) adminResult.launch(PairspanDeviceAdminReceiver.activationIntent(this@MainActivity))
                        else { PairspanDeviceAdminReceiver.deactivate(this@MainActivity); refreshPermissions() }
                    }
                    SwitchRow(stringResource(R.string.settings_start_boot), stringResource(R.string.settings_start_boot_d), startOnBoot, false) {
                        startOnBoot = it; prefs.edit().putBoolean("startOnBoot", it).apply()
                    }
                }
                Spacer(Modifier.height(20.dp))
                SectionTitle(stringResource(R.string.settings_section_ids))
                GroupCard {
                    val deviceId = remember { GatewaySocket.deviceId(this@MainActivity) }
                    val clientId = prefs.getString("clientId", "").orEmpty()
                    IdRow(stringResource(R.string.settings_device_id), deviceId, true)
                    IdRow(stringResource(R.string.settings_client_id), clientId.ifEmpty { "—" }, false)
                }
                Spacer(Modifier.height(20.dp))
                SectionTitle(stringResource(R.string.settings_section_about))
                GroupCard {
                    NavRow(stringResource(R.string.settings_share), stringResource(R.string.settings_share_d), true, ::shareApp)
                    NavRow(stringResource(R.string.settings_licenses), stringResource(R.string.settings_licenses_d), false) { screen = Screen.Licenses }
                }
                Spacer(Modifier.height(28.dp))
                Text(stringResource(R.string.settings_version_fmt, BuildConfig.VERSION_NAME), fontSize = 12.sp, color = Palette.Gray,
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.fillMaxWidth())
            }
        }
    }

    @Composable private fun LicensesContent() {
        val items = listOf(
            "ZXing core 3.4.1" to "Apache-2.0",
            "zxing-android-embedded 4.3.0" to "Apache-2.0",
            "libadb-android 3.1.1" to "Apache-2.0 / BSD-3-Clause / MIT",
            "OkHttp 4.12.0" to "Apache-2.0",
            "Conscrypt 2.5.3" to "Apache-2.0",
            "AndroidX & Jetpack Compose" to "Apache-2.0",
        )
        Column(Modifier.fillMaxSize().safeDrawingPadding()) {
            SubTopBar(stringResource(R.string.settings_licenses)) { screen = Screen.Settings }
            Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 20.dp).padding(bottom = 28.dp)) {
                GroupCard {
                    items.forEachIndexed { i, (name, license) ->
                        Column {
                            Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 14.dp)) {
                                Text(name, fontSize = 16.sp, fontWeight = FontWeight.Medium, color = Palette.OnSurface)
                                Text(license, fontSize = 13.sp, color = Palette.Gray, modifier = Modifier.padding(top = 3.dp))
                            }
                            if (i < items.lastIndex) RowDivider()
                        }
                    }
                }
            }
        }
    }

    @Composable private fun SubTopBar(title: String, onBack: () -> Unit) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = null, tint = Palette.Title)
            }
            Text(title, fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = Palette.Title, modifier = Modifier.padding(start = 4.dp))
        }
    }

    @Composable private fun SectionTitle(title: String) {
        Text(title, fontSize = 13.sp, fontWeight = FontWeight.SemiBold, color = Palette.Title, modifier = Modifier.padding(start = 4.dp, bottom = 10.dp, top = 4.dp))
    }

    @Composable private fun GroupCard(content: @Composable ColumnScope.() -> Unit) {
        val shape = RoundedCornerShape(18.dp)
        Column(Modifier.fillMaxWidth().clip(shape).background(Palette.Surface).border(1.dp, Palette.Border.copy(alpha = 0.65f), shape), content = content)
    }

    @Composable private fun RowDivider() {
        HorizontalDivider(Modifier.padding(horizontal = 16.dp), thickness = 1.dp, color = Palette.Border.copy(alpha = 0.55f))
    }

    @Composable private fun NavRow(title: String, subtitle: String, divider: Boolean, onClick: () -> Unit) {
        Column {
            Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(title, fontSize = 16.sp, fontWeight = FontWeight.Medium, color = Palette.OnSurface)
                    Text(subtitle, fontSize = 13.sp, color = Palette.Gray, modifier = Modifier.padding(top = 3.dp))
                }
                Spacer(Modifier.width(8.dp))
                Text(if (androidx.compose.ui.platform.LocalLayoutDirection.current == androidx.compose.ui.unit.LayoutDirection.Rtl) "‹" else "›",
                    fontSize = 22.sp, color = Palette.Secondary, fontWeight = FontWeight.Medium)
            }
            if (divider) RowDivider()
        }
    }

    @Composable private fun IdRow(title: String, value: String, divider: Boolean) {
        NavRow(title, value, divider) { if (value != "—") copyText(title, value) }
    }

    @Composable private fun SwitchRow(title: String, subtitle: String, checked: Boolean, divider: Boolean, onChange: (Boolean) -> Unit) {
        Column {
            Row(Modifier.fillMaxWidth().clickable { onChange(!checked) }.padding(horizontal = 16.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(title, fontSize = 16.sp, fontWeight = FontWeight.Medium, color = Palette.OnSurface)
                    Text(subtitle, fontSize = 13.sp, color = Palette.Gray, modifier = Modifier.padding(top = 3.dp))
                }
                Spacer(Modifier.width(8.dp))
                PillSwitch(checked)
            }
            if (divider) RowDivider()
        }
    }

    @Composable private fun PillSwitch(checked: Boolean) {
        val track by animateColorAsState(if (checked) Palette.Primary else Palette.Border, label = "track")
        val thumb by animateDpAsState(if (checked) 52.dp - 26.dp - 2.dp else 2.dp, label = "thumb")
        Box(Modifier.width(52.dp).height(30.dp).clip(RoundedCornerShape(50)).background(track), contentAlignment = Alignment.CenterStart) {
            Box(Modifier.offset(x = thumb).size(26.dp).shadow(if (checked) 1.dp else 2.dp, CircleShape).clip(CircleShape).background(Color.White))
        }
    }

    private object Palette {
        val Primary = Color(0xFF1C7655)
        val Title = Color(0xFF114A37)
        val Secondary = Color(0xFF5F725F)
        val Background = Color(0xFFFAFAFA)
        val Surface = Color.White
        val OnSurface = Color.Black
        val Border = Color(0xFFD2D2D2)
        val Gray = Color(0xFF808080)
    }

    @Composable private fun GatewayStatus() {
        val (label, color) = when (gatewayState) {
            GatewayProbe.OK -> stringResource(R.string.service_ok) to androidx.compose.ui.graphics.Color(0xFF1C7655)
            GatewayProbe.NO_LIVE_CONNECTIONS -> stringResource(R.string.service_limited) to androidx.compose.ui.graphics.Color(0xFFC77700)
            GatewayProbe.UNREACHABLE -> stringResource(R.string.service_down) to androidx.compose.ui.graphics.Color(0xFFB3261E)
            else -> stringResource(R.string.service_checking) to androidx.compose.ui.graphics.Color(0xFF8A8A8A)
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Surface(shape = androidx.compose.foundation.shape.CircleShape, color = color, modifier = Modifier.size(10.dp)) {}
            Spacer(Modifier.width(8.dp))
            Text(label, style = MaterialTheme.typography.bodyMedium)
        }
        Spacer(Modifier.height(14.dp))
    }

    @Composable private fun HomeScreen() {
        GatewayStatus()
        GroupCard {
            Column(Modifier.fillMaxWidth().padding(18.dp)) {
                val shown = when {
                    !enabled -> stringResource(R.string.status_disabled)
                    hasSession -> status.ifEmpty { stringResource(R.string.status_off) }
                    GatewayService.pairError.isNotEmpty() -> GatewayService.pairError
                    else -> stringResource(R.string.not_paired)
                }
                Text(shown, style = MaterialTheme.typography.titleLarge)
            }
        }
        Spacer(Modifier.height(20.dp))
        val setupDone = notificationGranted && cameraGranted && secureGranted && adbReady && gatewayState == GatewayProbe.OK
        if (!hasSession && !setupDone) SetupScreen()
        else if (!hasSession) {
            OutlinedTextField(value = code, onValueChange = { code = it.uppercase().filter { c -> c.isLetterOrDigit() }.take(20) },
                label = { Text(stringResource(R.string.code_label)) }, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii, capitalization = androidx.compose.ui.text.input.KeyboardCapitalization.Characters),
                singleLine = true, modifier = Modifier.fillMaxWidth())
            Spacer(Modifier.height(10.dp))
            Button(onClick = { pair(code) }, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.pair_connect)) }
            val scanPrompt = stringResource(R.string.scan_prompt)
            OutlinedButton(onClick = {
                scanner.launch(ScanOptions().setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                    .setCaptureActivity(PairspanCaptureActivity::class.java).setOrientationLocked(false)
                    .setPrompt(scanPrompt))
            }, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.scan_qr)) }
        } else OutlinedButton(onClick = ::disconnect, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.remove_connection)) }
    }

    private enum class StepState { Done, Running, Action, Optional }

    @Composable private fun SetupScreen() {
        val required = listOf(notificationGranted, cameraGranted, secureGranted, adbReady, gatewayState == GatewayProbe.OK)
        val grantCommand = "adb shell pm grant $packageName android.permission.WRITE_SECURE_SETTINGS"
        GroupCard {
            Column(Modifier.fillMaxWidth().padding(18.dp)) {
                Text(stringResource(R.string.setup_title), style = MaterialTheme.typography.titleLarge)
                Text(stringResource(R.string.setup_summary_fmt, required.count { it }, required.size), style = MaterialTheme.typography.bodySmall)
                Spacer(Modifier.height(10.dp))
                LinearProgressIndicator(progress = { required.count { it } / required.size.toFloat() }, modifier = Modifier.fillMaxWidth())
                Spacer(Modifier.height(8.dp))
                SetupRow(stringResource(R.string.step_notifications), stringResource(R.string.step_notifications_d),
                    if (notificationGranted) StepState.Done else StepState.Action, stringResource(R.string.action_allow), ::requestNotification)
                SetupRow(stringResource(R.string.step_camera), stringResource(R.string.step_camera_d),
                    if (cameraGranted) StepState.Done else StepState.Action, stringResource(R.string.action_allow), ::requestCamera)
                SetupRow(stringResource(R.string.step_admin), stringResource(R.string.step_admin_d),
                    if (adminActive) StepState.Done else StepState.Optional, stringResource(R.string.action_enable)) {
                    adminResult.launch(PairspanDeviceAdminReceiver.activationIntent(this@MainActivity))
                }
                SetupRow(stringResource(R.string.step_system),
                    if (secureGranted) stringResource(R.string.step_system_ok) else stringResource(R.string.step_system_usb),
                    if (secureGranted) StepState.Done else StepState.Action,
                    if (secureGranted) null else stringResource(R.string.action_copy_command)) { copyText("adb", grantCommand) }
                SetupRow(stringResource(R.string.step_services), stringResource(R.string.step_services_d),
                    if (servicesEnabled) StepState.Done else StepState.Optional,
                    if (servicesEnabled) null else stringResource(R.string.action_open_settings)) {
                    startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
                }
                SetupRow(stringResource(R.string.step_adb), if (adbReady) stringResource(R.string.step_adb_ok) else adbDetail,
                    when { adbReady -> StepState.Done; adbWaitTicks > 20 -> StepState.Action; else -> StepState.Running },
                    stringResource(R.string.action_retry)) { adbWaitTicks = 0; ShellBridge.reconnect(true) }
                if (!adbReady && adbWaitTicks > 20) {
                    Text(stringResource(R.string.adb_usb_hint), style = MaterialTheme.typography.bodySmall)
                }
                SetupRow(stringResource(R.string.step_service),
                    when (gatewayState) { GatewayProbe.OK -> stringResource(R.string.step_service_ok); GatewayProbe.NO_LIVE_CONNECTIONS -> stringResource(R.string.step_service_limited); GatewayProbe.UNREACHABLE -> stringResource(R.string.step_service_down); else -> stringResource(R.string.step_service_checking) },
                    when (gatewayState) { GatewayProbe.OK -> StepState.Done; GatewayProbe.UNREACHABLE, GatewayProbe.NO_LIVE_CONNECTIONS -> StepState.Action; else -> StepState.Running },
                    stringResource(R.string.action_retry)) { gatewayState = 0; GatewayProbe.check(this@MainActivity) { state -> runOnUiThread { gatewayState = state } } }
            }
        }
    }

    @Composable private fun SetupRow(title: String, detail: String, state: StepState, actionLabel: String? = null, onAction: (() -> Unit)? = null) {
        Row(Modifier.fillMaxWidth().padding(top = 12.dp), verticalAlignment = Alignment.Top) {
            Box(Modifier.size(26.dp), contentAlignment = Alignment.Center) {
                when (state) {
                    StepState.Running -> CircularProgressIndicator(strokeWidth = 2.dp, modifier = Modifier.size(18.dp))
                    StepState.Done -> Text("✓", color = androidx.compose.ui.graphics.Color(0xFF1C7655), style = MaterialTheme.typography.titleMedium)
                    StepState.Action -> Text("!", color = androidx.compose.ui.graphics.Color(0xFFB3261E), style = MaterialTheme.typography.titleMedium)
                    StepState.Optional -> Text("–", color = androidx.compose.ui.graphics.Color(0xFF8A8A8A), style = MaterialTheme.typography.titleMedium)
                }
            }
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.titleSmall)
                Text(detail, style = MaterialTheme.typography.bodySmall)
                if (state != StepState.Done && state != StepState.Running && actionLabel != null && onAction != null) {
                    TextButton(onClick = onAction, contentPadding = PaddingValues(0.dp)) { Text(actionLabel) }
                }
            }
        }
    }
}
