<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">
    <img src="web/assets/pairspan-icon.png" alt="Pairspan" width="100">
  </a>
</p>

<h1 align="center">Pairspan</h1>

<p align="center">Android telefonunuzu kablosuz ADB üzerinden macOS veya Windows’a bağlayın.</p>

<p align="center"><strong>Bir kez eşleştirin. Kablosuz bağlanın. Kolayca dağıtın.</strong></p>

<p align="center">
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0"><img src="https://img.shields.io/badge/release-v1.0.0-39795c" alt="Release v1.0.0"></a>
  <img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Android-39795c" alt="Platforms: macOS, Windows and Android">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-39795c" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">Web sitesi</a> ·
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0">İndir</a> ·
  <a href="#setup-guide">Kurulum rehberi</a> ·
  <a href="CONTRIBUTING.md">Katkıda bulun</a>
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
    <th align="center">Android · Bağlı</th>
    <th align="center">Android · Eşleştirme</th>
    <th align="center">macOS · QR kodu ve belirteç</th>
  </tr>
  <tr>
    <td align="center" valign="top"><img src="docs/screenshots/android-connected.png" alt="Android · Bağlı" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/android-pairing.png" alt="Android · Eşleştirme" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/macos-pairing.png" alt="macOS · QR kodu ve belirteç" width="300"></td>
  </tr>
</table>

## İndir

[Sürüm 1.0.0](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0): macOS, Windows ve Android.

## Klasörler

| Bileşen | Klasör |
| --- | --- |
| Android | [`android/`](android/) |
| macOS ve Windows | [`mac/`](mac/) |
| Web | [`web/`](web/) |

```bash
cd web && npm install && npm start
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

<a id="setup-guide"></a>

## Kurulum rehberi

Pairspan, masaüstü uygulaması (macOS veya Windows) ve Android uygulamasından oluşur. İkisi de `https://pairspan.ferhatozcelik.com` adresindeki ağ geçidine bağlanır; telefon ve bilgisayarın aynı ağda olması gerekmez. Bir bilgisayara birden fazla telefon bağlanabilir.

### 1. Masaüstü uygulamasını yükleyin

**macOS**

1. [Sürüm sayfasından](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0) `pairspan-mac-1.0.0.dmg` dosyasını indirin ve Pairspan’i Uygulamalar klasörüne sürükleyin.
2. Derleme Apple tarafından noter onaylı değildir. macOS ilk açılışı engellerse uygulamaya sağ tıklayıp **Aç** seçeneğini kullanın.
3. Pairspan menü çubuğunda çalışır. Eşleştirme belirtecini ve QR kodunu görmek için simgesine tıklayın.
4. `adb` uygulamayla birlikte gelir; ayrıca yüklemeniz gerekmez.

**Windows**

1. Aynı sürüm sayfasından `pairspan-windows-1.0.0.exe` dosyasını indirin ve çalıştırın.
2. Yükleyici kod imzalı değildir. SmartScreen uyarırsa **Ek bilgi → Yine de çalıştır** seçeneğini kullanın.
3. Pairspan sistem tepsisinde çalışır. Eşleştirme belirtecini ve QR kodunu görmek için simgesine tıklayın.
4. `adb.exe` uygulamayla birlikte gelir.

Pencerenin üstündeki anahtarla veya tepsi menüsündeki **Enabled** seçeneğiyle Pairspan’i açıp kapatabilirsiniz. Kapalıyken telefonlar bağlanamaz.

### 2. Android uygulamasını yükleyin

1. Sürüm sayfasından APK’yi indirip yükleyin. İstenirse tarayıcınıza veya dosya yöneticinize yükleme izni verin. Android 8.0 veya üzeri; kablosuz hata ayıklama için Android 11 veya üzeri gerekir.
2. Pairspan’i açın; istendiğinde bildirim ve kamera izinlerini verin. Kamera yalnızca QR kodu taramak için kullanılır.
3. **Ayarlar → Telefon hakkında** bölümünde **Yapım numarası** üzerine yedi kez dokunun. **Geliştirici seçenekleri** bölümünden **USB hata ayıklama** ve varsa **Kablosuz hata ayıklama** seçeneklerini açın.

### 3. Sistem erişimi verin (bir kez)

Pairspan, kendi erişilebilirlik ve bildirim hizmetlerini ve kablosuz ADB’yi Ayarlar’a yönlendirmeden açabilir. Bunun için bilgisayardan `WRITE_SECURE_SETTINGS` izni verilmelidir. Masaüstü uygulaması bunu sizin için yapar:

1. USB hata ayıklama açık ve Pairspan yüklü telefonunuzu USB kablosuyla bilgisayara bağlayın.
2. Masaüstündeki **One-time phone permission** bölümünde **Find USB phones** düğmesine tıklayın.
3. Telefonda USB hata ayıklamaya izin verme isteği çıkarsa onaylayın ve yeniden tarayın.
4. Telefonunuzun altındaki **Grant permission** düğmesine tıklayın. Uygulama, kendi `adb` aracıyla şu komutları çalıştırır ve kablosuz bağlantı için ADB’yi 5555 portuna geçirir:

```bash
adb shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS
adb tcpip 5555
```

5. Telefonda **İzin ver** seçeneğine dokunun ve **Her zaman izin ver** kutusunu işaretleyin. Komut yazmanız gerekmez.

Notlar:

- İzin, Pairspan kaldırılana kadar geçerlidir. 5555 portu ayarı telefon yeniden başlatılana kadar sürer; yeniden başlatma sonrasında tekrar **Grant permission** düğmesine tıklayın.
- Güvenlik hatası alırsanız Geliştirici seçeneklerinde **USB hata ayıklama (Güvenlik ayarları)** seçeneğini açıp yeniden deneyin (bazı Xiaomi/Redmi/POCO telefonlar).
- Komutu herhangi bir `adb` ile kendiniz de çalıştırabilirsiniz. Android uygulaması komutu **Getting ready → System access** altında **Copy command** düğmesiyle gösterir.
- Kablosuz hata ayıklama olmayan telefonlarda USB üzerinden bir kez `adb tcpip 5555` çalıştırın. Pairspan bu portu kullanır.

<a id="pair"></a>

### 4. Eşleştirin

1. Masaüstü uygulamasının açık olduğundan ve **Connected to the Pairspan service** mesajını gösterdiğinden emin olun.
2. Android uygulamasında **Scan QR code** düğmesine dokunup masaüstündeki kodu tarayın veya eşleştirme belirtecini yazın.
3. Telefon, masaüstünde **Connected devices** altında **Device ID** ve bilgisayarın **Client ID** bilgileriyle görünür. Aynı kimlikler Android uygulamasının **Settings** bölümündedir.
4. Başka bir telefon eklemek için yeni kodla eşleştirmeyi tekrarlayın. Kullanılan kod hemen yenilenir.

### 5. Kendi sunucunuzda barındırın

Masaüstü ve Android uygulamaları birbirlerine doğrudan ADB açmaz; ikisi de [`web/`](web/) içindeki ağ geçidine bağlanır. Genel adres `https://pairspan.ferhatozcelik.com`’dur. Kendi sunucunuzda bu süreci başlatın ve iki uygulamayı da aynı URL ve belirteçle derleyin.

### Gereksinimler

- Node.js 22 veya Docker.
- Ağ geçidi internete açıksa TLS destekli bir alan adı. İstemciler `https://` ve WebSocket için `wss://…/ws` kullanır.
- Sunucu, masaüstü ve Android uygulamalarında aynı `GATEWAY_TOKEN`.

Oturumlar bellekte tutulur ve süreç yeniden başlatıldığında silinir. Ağ geçidi `adb` çalıştırmaz; masaüstü uygulaması yalnızca `127.0.0.1` üzerinde çalıştırır.

### Ortam değişkenleri

`web/.env.example`, `mac/.env.example` ve `android/.env.example` aynı iki satırı kullanır. Genel URL’yi telefon ve bilgisayarların ulaşabildiği adres olarak ayarlayın. Git’te belirteci boş bırakın.

```text
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=
```

`GATEWAY_PUBLIC_URL`, eşleştirme QR kodlarına yazılır. `GATEWAY_TOKEN`, her `hello` mesajıyla gönderilir. Sunucu belirteci boşsa `/ws` adresine ulaşabilen herkes katılabilir. Sunucuyu internete açmadan önce uzun, rastgele bir belirteç oluşturun:

```bash
openssl rand -hex 32
```

Belirteci commit etmeyin. Sürecin ortam değişkenlerine veya Docker için `web/temp.env` dosyasına koyun. `temp.env` depoya dahil değildir.

### Node ile çalıştırın

```bash
cd web
npm install
export GATEWAY_PUBLIC_URL=https://pairspan.example.com
export GATEWAY_TOKEN='paste-the-token'
npm start
```

Süreç `0.0.0.0:3000` adresini dinler; açılış sayfasını, `/p/`, WebSocket `/ws` ve `GET /health` uç noktalarını sunar.

```bash
curl -fsS http://127.0.0.1:3000/health
```

Sağlıklı süreç `{"ok":true,"service":"pairspan",...}` döndürür.

### Docker ile çalıştırın

`web/` klasöründe:

```bash
docker build -t pairspan:latest .
```

`web/temp.env` dosyasını oluşturun:

```text
NODE_ENV=production
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=paste-the-token
```

[`web/docker-compose.yml`](web/docker-compose.yml), bu dosyayı okur ve bilgisayardaki `127.0.0.1:16200` adresini konteynerin `3000` portuna yönlendirir.

```bash
docker compose up -d
curl -fsS http://127.0.0.1:16200/health
```

`127.0.0.1:16200` önüne TLS destekli ters vekil sunucu koyun. `/ws` için WebSocket yükseltmelerini iletin. Genel adresi de kontrol edin:

```bash
curl -fsS https://pairspan.example.com/health
```

### Uygulamaları ağ geçidinize yönlendirin

Aynı URL ve belirteci `mac/.env.example` ve `android/.env.example` dosyalarına yazıp yeniden derleyin. Masaüstü uygulaması `.env.example` dosyasını açılışta okur; Android uygulaması iki değeri derleme sırasında APK’ye ekler.

```bash
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

Farklı belirteçle derlenen istemci reddedilir. QR kodunu taramadan önce masaüstünde **Connected to the Pairspan service** görünmelidir. Ardından [eşleştirme adımlarını](#pair) uygulayın.

## 6. Aktarımı kullanın

Her telefon, bilgisayarda `127.0.0.1:44755` adresinden başlayarak ayrı bir yerel port alır. Masaüstü penceresi her telefonun `adb connect` hedefini gösterir.

```bash
adb devices
adb -s 127.0.0.1:44755 shell id
```

`adb devices` çıktısında telefon `authorizing` görünüyorsa kilidini açıp USB hata ayıklama isteğini onaylayın ve **Her zaman izin ver** seçeneğini işaretleyin.

Tek telefonu ayırmak için Android uygulamasında **Remove connection** düğmesine dokunun. Tüm bağlantıları kesmek için masaüstündeki anahtarı kapatın.

## Lisans

[MIT](LICENSE)

## Güvenlik ve sorumlu kullanım

Pairspan’i yalnızca size ait veya erişim için açık izin aldığınız cihazlarda; meşru geliştirme, hata ayıklama ve cihaz yönetimi amacıyla kullanın. ADB’yi izinsiz erişim, gizli izleme, veri hırsızlığı, zararlı yazılım veya güvenlik önlemlerini aşmak için kullanmayın.

Yukarıdaki USB kurulumunda cihaz sahibi USB hata ayıklamayı açmalı, telefonun kilidini açmalı ve Android’in hata ayıklama yetkilendirme isteğini onaylamalıdır. Yalnızca güvendiğiniz bilgisayarlara izin verin. Eşleştirme ve otomatik yeniden bağlanma bu yetkilendirmeye bağlı olmalı; iptal edilen izinleri aşmamalıdır. Bkz. [Android ADB belgeleri](https://developer.android.com/tools/adb).

Eşleştirme belirteçlerini ve ADB anahtarlarını gizli tutun. Erişimi sonlandırmak için Pairspan bağlantısını kaldırın ve Android’in Geliştirici seçeneklerinden USB hata ayıklama yetkilerini iptal edin; ihtiyaç kalmadığında hata ayıklamayı kapatın. Android’in güvenlik ve izin modeline ve geçerli [Google Play Cihaz ve Ağ Kötüye Kullanımı politikasına](https://support.google.com/googleplay/android-developer/answer/16559646) uyun. USB üzerinden izin verilmesi tek başına Google Play politikalarına uygunluk anlamına gelmez.
