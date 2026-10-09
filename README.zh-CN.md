<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">
    <img src="web/assets/pairspan-icon.png" alt="Pairspan" width="100">
  </a>
</p>

<h1 align="center">Pairspan</h1>

<p align="center">通过无线 ADB 将 Android 手机连接到 macOS 或 Windows。</p>

<p align="center"><strong>配对一次，无线连接，轻松部署。</strong></p>

<p align="center">
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0"><img src="https://img.shields.io/badge/release-v1.0.0-39795c" alt="Release v1.0.0"></a>
  <img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Android-39795c" alt="Platforms: macOS, Windows and Android">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-39795c" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">网站</a> ·
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0">下载</a> ·
  <a href="#setup-guide">安装指南</a> ·
  <a href="CONTRIBUTING.md">参与贡献</a>
</p>

<p align="center">
  <a href="README.md">English</a> ·
  <a href="README.tr.md">Türkçe</a> ·
  <a href="README.de.md">Deutsch</a> ·
  <a href="README.es.md">Español</a> ·
  <a href="README.fr.md">Français</a> ·
  <a href="README.pt-BR.md">Português (Brasil)</a> ·
  <a href="README.zh-CN.md">简体中文</a> ·
  <a href="README.ja.md">日本語</a>
</p>

<br>

<table align="center">
  <tr>
    <th align="center">Android · 已连接</th>
    <th align="center">Android · 配对</th>
    <th align="center">macOS · 二维码和令牌</th>
  </tr>
  <tr>
    <td align="center" valign="top"><img src="docs/screenshots/android-connected.png" alt="Android · 已连接" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/android-pairing.png" alt="Android · 配对" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/macos-pairing.png" alt="macOS · 二维码和令牌" width="300"></td>
  </tr>
</table>

## 下载

[版本 1.0.0](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0)：macOS、Windows 和 Android。

## 目录

| 组件 | 目录 |
| --- | --- |
| Android | [`android/`](android/) |
| macOS 和 Windows | [`mac/`](mac/) |
| Web | [`web/`](web/) |

```bash
cd web && npm install && npm start
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

<a id="setup-guide"></a>

## 安装指南

Pairspan 包括桌面应用（macOS 或 Windows）和 Android 应用。两者都连接到 `https://pairspan.ferhatozcelik.com` 上的网关，因此手机和电脑无需位于同一网络。一台电脑可以同时连接多部手机。

### 1. 安装桌面应用

**macOS**

1. 从[版本发布页面](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0)下载 `pairspan-mac-1.0.0.dmg`，将 Pairspan 拖入“应用程序”文件夹。
2. 此版本未经 Apple 公证。如果 macOS 阻止首次启动，请右键点击 Pairspan，选择**打开**。
3. Pairspan 位于菜单栏中。点击图标即可查看配对令牌和二维码。
4. 已内置 `adb`，无需额外安装。

**Windows**

1. 从同一发布页面下载并运行 `pairspan-windows-1.0.0.exe`。
2. 安装程序没有代码签名。如果 SmartScreen 显示警告，请选择**更多信息 → 仍要运行**。
3. Pairspan 位于系统托盘中。点击图标即可查看配对令牌和二维码。
4. 已内置 `adb.exe`。

使用窗口顶部的开关或托盘菜单中的 **Enabled** 启用或停用 Pairspan。停用期间，手机无法连接。

### 2. 安装 Android 应用

1. 从发布页面下载并安装 APK。根据提示允许浏览器或文件管理器安装应用。需要 Android 8.0 或更高版本；无线调试需要 Android 11 或更高版本。
2. 打开 Pairspan，按提示授予通知和相机权限。相机仅用于扫描二维码。
3. 在手机的**设置 → 关于手机**中连续点击**版本号**七次，然后进入**开发者选项**，启用 **USB 调试**；如果支持，也启用**无线调试**。

### 3. 授予系统访问权限（仅需一次）

Pairspan 可以直接启用自己的无障碍服务、通知服务和无线 ADB，无需手动进入设置。但 Android 要求先通过电脑授予 `WRITE_SECURE_SETTINGS` 权限。桌面应用会代为完成：

1. 用 USB 线连接手机与电脑，确保已启用 USB 调试且手机已安装 Pairspan。
2. 在桌面的 **One-time phone permission** 区域点击 **Find USB phones**。
3. 如果手机提示允许 USB 调试，请确认并重新扫描。
4. 点击手机下方的 **Grant permission**。应用使用内置的 `adb` 执行以下命令，并将手机的 ADB 切换到 5555 端口，以便无需线缆连接：

```bash
adb shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS
adb tcpip 5555
```

5. 在手机上点击**允许**，并勾选**始终允许**。无需手动输入命令。

注意：

- 权限会一直保留到卸载 Pairspan 为止。5555 端口设置在手机重启后失效；重启后请再次点击 **Grant permission**。
- 如果出现安全异常，请在开发者选项中启用 **USB 调试（安全设置）**并重试（部分 Xiaomi/Redmi/POCO 手机需要此设置）。
- 也可以使用任意 `adb` 手动执行命令。Android 应用在 **Getting ready → System access** 中显示该命令，并提供 **Copy command** 按钮。
- 对于不支持无线调试的手机，通过 USB 执行一次 `adb tcpip 5555`。Pairspan 会使用此端口。

<a id="pair"></a>

### 4. 配对

1. 确保桌面应用已启用，并显示 **Connected to the Pairspan service**。
2. 在 Android 应用中点击 **Scan QR code**，扫描电脑上的二维码，或输入配对令牌。
3. 手机会出现在桌面的 **Connected devices** 中，同时显示手机的 **Device ID** 和电脑的 **Client ID**。Android 应用的 **Settings** 中也能查看这两个标识符。
4. 要添加另一部手机，请使用新二维码重复配对。已使用的码会立即被替换。

### 5. 自行托管

桌面应用与 Android 应用不会直接向彼此开放 ADB。两者都连接到 [`web/`](web/) 中的网关。公共服务地址是 `https://pairspan.ferhatozcelik.com`。如需自行托管，请启动该进程，并使用相同的 URL 和令牌构建两个应用。

### 环境要求

- Node.js 22 或 Docker。
- 如果网关可从互联网访问，需要支持 TLS 的主机名。客户端使用 `https://` 和 WebSocket `wss://…/ws`。
- 服务器、桌面应用和 Android 应用使用相同的 `GATEWAY_TOKEN`。

会话保存在内存中，重启进程会清除会话。网关不会运行 `adb`；桌面应用仅在 `127.0.0.1` 上运行它。

### 环境变量

`web/.env.example`、`mac/.env.example` 和 `android/.env.example` 使用相同的两行配置。将公共 URL 设置为手机和电脑都能访问的地址。在 Git 中将令牌留空。

```text
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=
```

`GATEWAY_PUBLIC_URL` 会写入配对二维码。每条 `hello` 消息都会发送 `GATEWAY_TOKEN`。如果服务器令牌为空，任何能访问 `/ws` 的人都可以加入。在向互联网开放进程之前，请生成足够长的随机令牌：

```bash
openssl rand -hex 32
```

不要将令牌提交到 Git。将其放入进程环境变量中，或在使用 Docker 时放入 `web/temp.env`。`temp.env` 不属于仓库内容。

### 使用 Node 运行

```bash
cd web
npm install
export GATEWAY_PUBLIC_URL=https://pairspan.example.com
export GATEWAY_TOKEN='paste-the-token'
npm start
```

进程监听 `0.0.0.0:3000`，提供首页、`/p/`、WebSocket `/ws` 和 `GET /health`。

```bash
curl -fsS http://127.0.0.1:3000/health
```

正常运行时返回 `{"ok":true,"service":"pairspan",...}`。

### 使用 Docker 运行

在 `web/` 目录中执行：

```bash
docker build -t pairspan:latest .
```

创建 `web/temp.env`：

```text
NODE_ENV=production
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=paste-the-token
```

[`web/docker-compose.yml`](web/docker-compose.yml) 读取此文件，将容器的 `3000` 端口映射到 `127.0.0.1:16200`。

```bash
docker compose up -d
curl -fsS http://127.0.0.1:16200/health
```

在 `127.0.0.1:16200` 前配置支持 TLS 的反向代理，并转发 `/ws` 上的 WebSocket 升级请求。同样检查公共 URL：

```bash
curl -fsS https://pairspan.example.com/health
```

### 将应用指向自己的网关

将相同的 URL 和令牌写入 `mac/.env.example` 和 `android/.env.example`，然后重新构建。桌面应用在启动时读取 `.env.example`；Android 应用在编译时将这两个值写入 APK。

```bash
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

使用不同令牌构建的客户端会被拒绝。扫描二维码前，桌面应用应显示 **Connected to the Pairspan service**。然后按照[配对步骤](#pair)操作。

## 6. 使用中继

每部手机在电脑上都有独立的本地端口，从 `127.0.0.1:44755` 开始递增。桌面窗口显示每部手机的 `adb connect` 目标地址。

```bash
adb devices
adb -s 127.0.0.1:44755 shell id
```

如果 `adb devices` 显示手机状态为 `authorizing`，请解锁手机，允许 USB 调试，并勾选**始终允许**。

要断开单部手机，请在 Android 应用中点击 **Remove connection**。要断开所有连接，请关闭桌面开关。

## 许可证

[MIT](LICENSE)

## 安全与负责任的使用

仅在您拥有或已获得明确访问许可的设备上使用 Pairspan，用于合法的开发、调试和设备管理。请勿使用 ADB 进行未经授权的访问、隐蔽监控、数据窃取、传播恶意软件或绕过安全措施。

在上述 USB 设置过程中，设备所有者必须启用 USB 调试、解锁手机并确认 Android 的调试授权提示。仅授权可信任的电脑。配对和自动重新连接必须遵守此授权，不得绕过已撤销的权限。请参阅 [Android ADB 文档](https://developer.android.com/tools/adb)。

妥善保管配对令牌和 ADB 密钥。要终止访问，请移除 Pairspan 连接，并在 Android 开发者选项中撤销 USB 调试授权；不再需要调试时，请将其关闭。遵守 Android 的安全与权限模型，以及适用的 [Google Play 设备和网络滥用政策](https://support.google.com/googleplay/android-developer/answer/16559646)。仅获得 USB 授权并不代表符合 Google Play 政策。
