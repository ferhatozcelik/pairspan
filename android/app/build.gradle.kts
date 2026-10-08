import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

kotlin { compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) } }

val localEnv = Properties().apply {
    val localFile = rootProject.file(".env.example")
    if (localFile.exists()) localFile.inputStream().use { load(it) }
}

fun env(name: String): String =
    (System.getenv(name) ?: localEnv.getProperty(name) ?: "").trim()

val gatewayUrl = (env("GATEWAY_PUBLIC_URL").ifEmpty { env("GATEWAY_URL") }.ifEmpty { "https://pairspan.ferhatozcelik.com" }).replace("\"", "\\\"")
val gatewayToken = env("GATEWAY_TOKEN").replace("\"", "\\\"")
val keystoreFileName = env("KEYSTORE_FILE")
val releaseStorePassword = env("KEYSTORE_PASSWORD")
val releaseKeyAlias = env("KEY_ALIAS")
val releaseKeyPassword = env("KEY_PASSWORD")
val keystoreFile = if (keystoreFileName.isNotEmpty()) rootProject.file(keystoreFileName) else null
val hasReleaseKeystore = keystoreFile != null && keystoreFile.isFile && releaseStorePassword.isNotEmpty() && releaseKeyAlias.isNotEmpty() && releaseKeyPassword.isNotEmpty()

android {
    namespace = "com.pairspan.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.pairspan"
        minSdk = 26
        targetSdk = 35
        versionCode = 5
        versionName = "1.0.0"
        buildConfigField("String", "GATEWAY_URL", "\"$gatewayUrl\"")
        buildConfigField("String", "GATEWAY_TOKEN", "\"$gatewayToken\"")
    }

    signingConfigs {
        if (hasReleaseKeystore) {
            create("release") {
                storeFile = keystoreFile
                storePassword = releaseStorePassword
                keyAlias = releaseKeyAlias
                keyPassword = releaseKeyPassword
            }
        }
    }

    buildTypes {
        release {
            signingConfig = if (hasReleaseKeystore) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug")
            }
        }
    }

    buildFeatures { buildConfig = true; compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.05.00")
    implementation(composeBom)
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.conscrypt:conscrypt-android:2.5.3")
    implementation("androidx.annotation:annotation:1.9.1")
}
