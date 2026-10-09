<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">
    <img src="web/assets/pairspan-icon.png" alt="Pairspan" width="100">
  </a>
</p>

<h1 align="center">Pairspan</h1>

<p align="center">ワイヤレス ADB で Android スマートフォンを macOS または Windows に接続。</p>

<p align="center"><strong>一度ペアリング。ワイヤレスで接続。手軽にデプロイ。</strong></p>

<p align="center">
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0"><img src="https://img.shields.io/badge/release-v1.0.0-39795c" alt="Release v1.0.0"></a>
  <img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Android-39795c" alt="Platforms: macOS, Windows and Android">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-39795c" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">ウェブサイト</a> ·
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0">ダウンロード</a> ·
  <a href="#setup-guide">セットアップガイド</a> ·
  <a href="CONTRIBUTING.md">貢献する</a>
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
    <th align="center">Android · 接続済み</th>
    <th align="center">Android · ペアリング</th>
    <th align="center">macOS · QR コードとトークン</th>
  </tr>
  <tr>
    <td align="center" valign="top"><img src="docs/screenshots/android-connected.png" alt="Android · 接続済み" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/android-pairing.png" alt="Android · ペアリング" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/macos-pairing.png" alt="macOS · QR コードとトークン" width="300"></td>
  </tr>
</table>

## ダウンロード

[バージョン 1.0.0](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0)：macOS、Windows、Android。

## ディレクトリ

| コンポーネント | ディレクトリ |
| --- | --- |
| Android | [`android/`](android/) |
| macOS と Windows | [`mac/`](mac/) |
| Web | [`web/`](web/) |

```bash
cd web && npm install && npm start
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

<a id="setup-guide"></a>

## セットアップガイド

Pairspan はデスクトップアプリ（macOS または Windows）と Android アプリで構成されています。どちらも `https://pairspan.ferhatozcelik.com` のゲートウェイに接続するため、スマートフォンとコンピューターが同じネットワークにある必要はありません。1 台のコンピューターに複数のスマートフォンを同時に接続できます。

### 1. デスクトップアプリをインストール

**macOS**

1. [リリースページ](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0)から `pairspan-mac-1.0.0.dmg` をダウンロードし、Pairspan を「アプリケーション」にドラッグします。
2. このビルドは Apple の公証を受けていません。初回起動がブロックされた場合は、Pairspan を右クリックして**開く**を選択します。
3. Pairspan はメニューバーに常駐します。アイコンをクリックすると、ペアリングトークンと QR コードが表示されます。
4. `adb` は同梱されているため、追加インストールは不要です。

**Windows**

1. 同じリリースページから `pairspan-windows-1.0.0.exe` をダウンロードして実行します。
2. インストーラーにはコード署名がありません。SmartScreen が警告した場合は、**詳細情報 → 実行**を選択します。
3. Pairspan はシステムトレイに常駐します。アイコンをクリックすると、トークンと QR コードが表示されます。
4. `adb.exe` は同梱されています。

ウィンドウ上部のスイッチ、またはトレイメニューの **Enabled** で Pairspan を有効・無効にできます。無効の間はスマートフォンを接続できません。

### 2. Android アプリをインストール

1. リリースページから APK をダウンロードしてインストールします。必要に応じて、ブラウザーやファイルマネージャーからのインストールを許可してください。Android 8.0 以降が必要で、ワイヤレスデバッグには Android 11 以降が必要です。
2. Pairspan を開き、通知とカメラの権限を許可します。カメラは QR コードの読み取りにのみ使用します。
3. **設定 → デバイス情報**で**ビルド番号**を 7 回タップします。**開発者向けオプション**で **USB デバッグ**を有効にし、対応している場合は**ワイヤレスデバッグ**も有効にします。

### 3. システムアクセスを許可（初回のみ）

Pairspan は設定画面を開かずに、自身のユーザー補助サービス、通知サービス、ワイヤレス ADB を有効にできます。ただし、Android では事前にコンピューターから `WRITE_SECURE_SETTINGS` 権限を付与する必要があります。デスクトップアプリで実行できます。

1. USB デバッグを有効にし、Pairspan をインストールしたスマートフォンを USB ケーブルで接続します。
2. デスクトップの **One-time phone permission** で **Find USB phones** をクリックします。
3. スマートフォンに USB デバッグの許可確認が表示された場合は許可し、もう一度検索します。
4. 対象のスマートフォンの下にある **Grant permission** をクリックします。アプリは同梱の `adb` で次のコマンドを実行し、ケーブルなしで接続できるように ADB をポート 5555 に切り替えます。

```bash
adb shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS
adb tcpip 5555
```

5. スマートフォンで**許可**をタップし、**常に許可する**にチェックを入れます。コマンドの手入力は不要です。

注意事項：

- 権限は Pairspan をアンインストールするまで有効です。ポート 5555 の設定はスマートフォンの再起動で解除されるため、再起動後はもう一度 **Grant permission** をクリックしてください。
- セキュリティ例外が発生した場合は、開発者向けオプションで **USB デバッグ（セキュリティ設定）**を有効にして再試行します（一部の Xiaomi/Redmi/POCO 端末）。
- 任意の `adb` を使って手動で実行することもできます。Android アプリの **Getting ready → System access** にコマンドと **Copy command** ボタンがあります。
- ワイヤレスデバッグに対応していない端末では、USB 経由で一度 `adb tcpip 5555` を実行します。Pairspan はこのポートを使用します。

<a id="pair"></a>

### 4. ペアリング

1. デスクトップアプリが有効で、**Connected to the Pairspan service** が表示されていることを確認します。
2. Android アプリで **Scan QR code** をタップしてデスクトップのコードを読み取るか、ペアリングトークンを入力します。
3. デスクトップの **Connected devices** にスマートフォンの **Device ID** とコンピューターの **Client ID** が表示されます。同じ ID は Android アプリの **Settings** にもあります。
4. 別のスマートフォンを追加するには、新しいコードでペアリングを繰り返します。使用済みのコードはすぐに置き換わります。

### 5. セルフホスティング

デスクトップアプリと Android アプリは、互いに ADB を直接公開しません。どちらも [`web/`](web/) のゲートウェイに接続します。公開サービスは `https://pairspan.ferhatozcelik.com` です。自分でホストする場合は、そのプロセスを起動し、両アプリを同じ URL とトークンでビルドしてください。

### 要件

- Node.js 22 または Docker。
- ゲートウェイをインターネットから公開する場合は、TLS 対応のホスト名。クライアントは `https://` と WebSocket `wss://…/ws` を使用します。
- サーバー、デスクトップアプリ、Android アプリで共通の `GATEWAY_TOKEN`。

セッションはメモリーに保持され、プロセスの再起動で失われます。ゲートウェイは `adb` を実行しません。デスクトップアプリが `127.0.0.1` のみで実行します。

### 環境変数

`web/.env.example`、`mac/.env.example`、`android/.env.example` は同じ 2 行を使用します。公開 URL はスマートフォンとコンピューターからアクセスできるアドレスにしてください。Git に保存するトークンは空にします。

```text
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=
```

`GATEWAY_PUBLIC_URL` はペアリング用 QR コードに書き込まれます。`GATEWAY_TOKEN` は各 `hello` メッセージで送信されます。サーバーのトークンが空の場合、`/ws` にアクセスできる人は誰でも参加できます。インターネットへ公開する前に、長いランダムなトークンを生成してください。

```bash
openssl rand -hex 32
```

トークンをコミットしないでください。実行プロセスの環境変数、または Docker 用の `web/temp.env` に保存します。`temp.env` はリポジトリに含まれません。

### Node で実行

```bash
cd web
npm install
export GATEWAY_PUBLIC_URL=https://pairspan.example.com
export GATEWAY_TOKEN='paste-the-token'
npm start
```

プロセスは `0.0.0.0:3000` で待ち受け、トップページ、`/p/`、WebSocket `/ws`、`GET /health` を提供します。

```bash
curl -fsS http://127.0.0.1:3000/health
```

正常なプロセスは `{"ok":true,"service":"pairspan",...}` を返します。

### Docker で実行

`web/` ディレクトリで実行します。

```bash
docker build -t pairspan:latest .
```

`web/temp.env` を作成します。

```text
NODE_ENV=production
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=paste-the-token
```

[`web/docker-compose.yml`](web/docker-compose.yml) はこのファイルを読み取り、コンテナーのポート `3000` を `127.0.0.1:16200` に公開します。

```bash
docker compose up -d
curl -fsS http://127.0.0.1:16200/health
```

`127.0.0.1:16200` の前に TLS 対応のリバースプロキシを配置し、`/ws` の WebSocket アップグレード要求を転送します。公開 URL も確認してください。

```bash
curl -fsS https://pairspan.example.com/health
```

### アプリを自分のゲートウェイに接続

同じ URL とトークンを `mac/.env.example` と `android/.env.example` に書き込み、再ビルドします。デスクトップアプリは起動時に `.env.example` を読み込み、Android アプリはビルド時に両方の値を APK に組み込みます。

```bash
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

異なるトークンでビルドしたクライアントは拒否されます。QR コードを読み取る前に、デスクトップに **Connected to the Pairspan service** が表示されていることを確認し、[ペアリング手順](#pair)に進んでください。

## 6. リレーを使う

各スマートフォンには、コンピューター上で `127.0.0.1:44755` から順に独立したローカルポートが割り当てられます。デスクトップウィンドウには、端末ごとの `adb connect` 接続先が表示されます。

```bash
adb devices
adb -s 127.0.0.1:44755 shell id
```

`adb devices` に `authorizing` と表示された場合は、スマートフォンのロックを解除し、USB デバッグを許可して**常に許可する**にチェックを入れます。

1 台だけ切断するには、Android アプリで **Remove connection** をタップします。すべて切断するには、デスクトップのスイッチをオフにします。

## ライセンス

[MIT](LICENSE)
