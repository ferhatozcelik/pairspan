package com.pairspan.app

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import okhttp3.Call
import okhttp3.Callback
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.util.concurrent.TimeUnit

data class UpdateReleaseInfo(
    val versionName: String,
    val tagName: String,
    val releaseTitle: String,
    val releaseNotes: String,
    val downloadUrl: String,
    val apkSize: Long
)

enum class UpdateStatus {
    IDLE,
    CHECKING,
    UPDATE_AVAILABLE,
    UP_TO_DATE,
    DOWNLOADING,
    READY_TO_INSTALL,
    ERROR
}

object OtaUpdateManager {
    private const val GITHUB_RELEASES_URL = "https://api.github.com/repos/ferhatozcelik/pairspan/releases/latest"
    private const val FALLBACK_APK_URL = "https://github.com/ferhatozcelik/pairspan/releases/download/v1.0.0/pairspan-android-1.0.0.apk"

    private val httpClient = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(60, TimeUnit.SECONDS)
        .followRedirects(true)
        .followSslRedirects(true)
        .build()

    fun isNewerVersion(remote: String, local: String): Boolean {
        val cleanRemote = remote.trim().removePrefix("v").removePrefix("V")
        val cleanLocal = local.trim().removePrefix("v").removePrefix("V")
        val rParts = cleanRemote.split(".").map { it.toIntOrNull() ?: 0 }
        val lParts = cleanLocal.split(".").map { it.toIntOrNull() ?: 0 }
        val maxLen = maxOf(rParts.size, lParts.size)
        for (i in 0 until maxLen) {
            val r = rParts.getOrElse(i) { 0 }
            val l = lParts.getOrElse(i) { 0 }
            if (r > l) return true
            if (r < l) return false
        }
        return false
    }

    fun checkForUpdates(
        currentVersion: String = BuildConfig.VERSION_NAME,
        onComplete: (status: UpdateStatus, info: UpdateReleaseInfo?, errorMsg: String?) -> Unit
    ) {
        val request = Request.Builder()
            .url(GITHUB_RELEASES_URL)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "Pairspan-Android-OTA")
            .build()

        httpClient.newCall(request).enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) {
                onComplete(UpdateStatus.ERROR, null, e.localizedMessage ?: "Network connection failed")
            }

            override fun onResponse(call: Call, response: Response) {
                response.use { res ->
                    if (!res.isSuccessful) {
                        onComplete(UpdateStatus.ERROR, null, "GitHub HTTP ${res.code}")
                        return
                    }
                    val body = res.body?.string() ?: ""
                    try {
                        val json = JSONObject(body)
                        val tagName = json.optString("tag_name", "")
                        val title = json.optString("name", tagName)
                        val notes = json.optString("body", "")
                        val cleanVersion = tagName.trim().removePrefix("v").removePrefix("V")

                        var apkUrl = ""
                        var apkSize = 0L

                        val assets = json.optJSONArray("assets")
                        if (assets != null) {
                            for (i in 0 until assets.length()) {
                                val asset = assets.getJSONObject(i)
                                val name = asset.optString("name", "")
                                if (name.endsWith(".apk", ignoreCase = true)) {
                                    apkUrl = asset.optString("browser_download_url", "")
                                    apkSize = asset.optLong("size", 0L)
                                    break
                                }
                            }
                        }

                        if (apkUrl.isEmpty()) {
                            apkUrl = FALLBACK_APK_URL
                        }

                        val info = UpdateReleaseInfo(
                            versionName = cleanVersion.ifEmpty { "1.0.0" },
                            tagName = tagName,
                            releaseTitle = title,
                            releaseNotes = notes,
                            downloadUrl = apkUrl,
                            apkSize = apkSize
                        )

                        if (isNewerVersion(cleanVersion, currentVersion)) {
                            onComplete(UpdateStatus.UPDATE_AVAILABLE, info, null)
                        } else {
                            onComplete(UpdateStatus.UP_TO_DATE, info, null)
                        }
                    } catch (e: Exception) {
                        onComplete(UpdateStatus.ERROR, null, "Failed to parse release: ${e.message}")
                    }
                }
            }
        })
    }

    fun downloadApk(
        context: Context,
        info: UpdateReleaseInfo,
        onProgress: (Int) -> Unit,
        onComplete: (File) -> Unit,
        onError: (String) -> Unit
    ) {
        val updatesDir = File(context.cacheDir, "updates")
        if (!updatesDir.exists()) updatesDir.mkdirs()

        val destFile = File(updatesDir, "pairspan-${info.versionName}.apk")
        if (destFile.exists() && destFile.length() > 1000000L && (info.apkSize <= 0 || destFile.length() == info.apkSize)) {
            onProgress(100)
            onComplete(destFile)
            return
        }

        val request = Request.Builder()
            .url(info.downloadUrl)
            .header("User-Agent", "Pairspan-Android-OTA")
            .build()

        httpClient.newCall(request).enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) {
                onError(e.localizedMessage ?: "Download failed")
            }

            override fun onResponse(call: Call, response: Response) {
                response.use { res ->
                    if (!res.isSuccessful) {
                        onError("Download failed: HTTP ${res.code}")
                        return
                    }
                    val body = res.body ?: run {
                        onError("Empty response body")
                        return
                    }

                    val totalBytes = body.contentLength()
                    var downloadedBytes = 0L

                    try {
                        val tempFile = File(updatesDir, "pairspan-${info.versionName}.tmp")
                        body.byteStream().use { input ->
                            FileOutputStream(tempFile).use { output ->
                                val buffer = ByteArray(8 * 1024)
                                var read: Int
                                var lastReportedPercent = -1
                                while (input.read(buffer).also { read = it } != -1) {
                                    output.write(buffer, 0, read)
                                    downloadedBytes += read
                                    if (totalBytes > 0) {
                                        val percent = ((downloadedBytes * 100) / totalBytes).toInt()
                                        if (percent != lastReportedPercent) {
                                            lastReportedPercent = percent
                                            onProgress(percent)
                                        }
                                    }
                                }
                                output.flush()
                            }
                        }

                        if (tempFile.renameTo(destFile) || (destFile.delete() && tempFile.renameTo(destFile))) {
                            onProgress(100)
                            onComplete(destFile)
                        } else {
                            onError("Failed to finalize downloaded APK file")
                        }
                    } catch (e: Exception) {
                        onError("Write error: ${e.message}")
                    }
                }
            }
        })
    }

    fun canInstallDirectly(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            context.packageManager.canRequestPackageInstalls()
        } else {
            true
        }
    }

    fun requestInstallPermission(context: Context) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
                data = Uri.parse("package:${context.packageName}")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            context.startActivity(intent)
        }
    }

    fun triggerInstall(context: Context, apkFile: File) {
        if (!apkFile.exists()) return

        val uri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            apkFile
        )

        val installIntent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }

        context.startActivity(installIntent)
    }
}
