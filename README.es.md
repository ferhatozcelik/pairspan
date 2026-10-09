<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">
    <img src="web/assets/pairspan-icon.png" alt="Pairspan" width="100">
  </a>
</p>

<h1 align="center">Pairspan</h1>

<p align="center">Conecta tu teléfono Android a macOS o Windows mediante ADB inalámbrico.</p>

<p align="center"><strong>Empareja una vez. Conecta sin cables. Despliega fácilmente.</strong></p>

<p align="center">
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0"><img src="https://img.shields.io/badge/release-v1.0.0-39795c" alt="Release v1.0.0"></a>
  <img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Android-39795c" alt="Platforms: macOS, Windows and Android">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-39795c" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">Sitio web</a> ·
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0">Descargar</a> ·
  <a href="#setup-guide">Guía de instalación</a> ·
  <a href="CONTRIBUTING.md">Contribuir</a>
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

<p align="center">
  <strong>Apoya el proyecto</strong><br>
  <a href="https://buymeacoffee.com/ferhatozcelik">Buy Me a Coffee</a> ·
  <a href="https://thanks.dev/donations">thanks.dev</a>
</p>

<br>

<table align="center">
  <tr>
    <th align="center">Android · Conectado</th>
    <th align="center">Android · Emparejamiento</th>
    <th align="center">macOS · Código QR y token</th>
  </tr>
  <tr>
    <td align="center" valign="top"><img src="docs/screenshots/android-connected.png" alt="Android · Conectado" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/android-pairing.png" alt="Android · Emparejamiento" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/macos-pairing.png" alt="macOS · Código QR y token" width="300"></td>
  </tr>
</table>

## Descargar

[Versión 1.0.0](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0): macOS, Windows y Android.

## Carpetas

| Componente | Carpeta |
| --- | --- |
| Android | [`android/`](android/) |
| macOS y Windows | [`mac/`](mac/) |
| Web | [`web/`](web/) |

```bash
cd web && npm install && npm start
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

<a id="setup-guide"></a>

## Guía de instalación

Pairspan consta de una aplicación de escritorio (macOS o Windows) y otra para Android. Ambas se conectan a la pasarela en `https://pairspan.ferhatozcelik.com`; el teléfono y el ordenador no necesitan estar en la misma red. Un ordenador puede conectar varios teléfonos simultáneamente.

### 1. Instalar la aplicación de escritorio

**macOS**

1. Descarga `pairspan-mac-1.0.0.dmg` de la [página de versiones](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0) y arrastra Pairspan a Aplicaciones.
2. La aplicación no está notarizada por Apple. Si macOS bloquea el primer inicio, haz clic derecho en Pairspan y selecciona **Abrir**.
3. Pairspan aparece en la barra de menús. Pulsa su icono para ver el token de emparejamiento y el código QR.
4. `adb` está incluido; no necesitas instalar nada más.

**Windows**

1. Descarga `pairspan-windows-1.0.0.exe` de la misma página y ejecútalo.
2. El instalador no tiene firma de código. Si SmartScreen muestra una advertencia, selecciona **Más información → Ejecutar de todas formas**.
3. Pairspan aparece en la bandeja del sistema. Pulsa su icono para ver el token y el código QR.
4. `adb.exe` está incluido.

Activa o desactiva Pairspan con el interruptor de la cabecera o **Enabled** en el menú de la bandeja. Mientras esté desactivado, ningún teléfono podrá conectarse.

### 2. Instalar la aplicación Android

1. Descarga e instala el APK de la página de versiones. Autoriza las instalaciones desde el navegador o el gestor de archivos si se solicita. Se necesita Android 8.0 o posterior, y Android 11 o posterior para la depuración inalámbrica.
2. Abre Pairspan y concede los permisos de notificaciones y cámara. La cámara solo se utiliza para escanear el código QR.
3. En **Ajustes → Acerca del teléfono**, pulsa siete veces **Número de compilación**. En **Opciones de desarrollador**, activa **Depuración USB** y, si está disponible, **Depuración inalámbrica**.

### 3. Conceder acceso al sistema (una sola vez)

Pairspan puede activar sus servicios de accesibilidad y notificaciones, así como ADB inalámbrico, sin pasar por Ajustes. Android exige conceder `WRITE_SECURE_SETTINGS` desde un ordenador. La aplicación de escritorio lo hace por ti:

1. Conecta el teléfono por USB con la depuración USB activada y Pairspan instalado.
2. En **One-time phone permission**, pulsa **Find USB phones**.
3. Si el teléfono solicita permiso para depuración USB, acéptalo y vuelve a buscar.
4. Pulsa **Grant permission** debajo de tu teléfono. La aplicación ejecuta estos comandos con su `adb` incluido y cambia ADB al puerto 5555 para conectar sin cable:

```bash
adb shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS
adb tcpip 5555
```

5. En el teléfono, pulsa **Permitir** y marca **Permitir siempre**. No necesitas escribir comandos.

Notas:

- El permiso dura hasta que se desinstala Pairspan. La configuración del puerto 5555 dura hasta que se reinicia el teléfono; después pulsa de nuevo **Grant permission**.
- Si aparece una excepción de seguridad, activa **Depuración USB (ajustes de seguridad)** en las opciones de desarrollador y vuelve a intentarlo (algunos Xiaomi/Redmi/POCO).
- También puedes ejecutar el comando con cualquier `adb`. La aplicación Android lo muestra en **Getting ready → System access**, con el botón **Copy command**.
- En teléfonos sin depuración inalámbrica, ejecuta una vez `adb tcpip 5555` por USB. Pairspan usará ese puerto.

<a id="pair"></a>

### 4. Emparejar

1. Comprueba que la aplicación de escritorio esté activada y muestre **Connected to the Pairspan service**.
2. En Android, pulsa **Scan QR code** y escanea el código del ordenador o escribe el token.
3. El teléfono aparece en **Connected devices**, con su **Device ID** y el **Client ID** del ordenador. Ambos identificadores también aparecen en **Settings** en Android.
4. Para añadir otro teléfono, repite el emparejamiento con el nuevo código. Cada código usado se sustituye inmediatamente.

### 5. Alojar tu propia pasarela

Las aplicaciones de escritorio y Android no exponen ADB directamente entre sí. Ambas se conectan a la pasarela de [`web/`](web/). El servicio público está en `https://pairspan.ferhatozcelik.com`. Para alojarlo tú mismo, inicia ese proceso y compila ambas aplicaciones con la misma URL y el mismo token.

### Requisitos

- Node.js 22 o Docker.
- Un nombre de host con TLS si la pasarela es accesible desde Internet. Los clientes usan `https://` y WebSocket `wss://…/ws`.
- El mismo `GATEWAY_TOKEN` en el servidor y ambas aplicaciones.

Las sesiones se guardan en memoria y se pierden al reiniciar el proceso. La pasarela no ejecuta `adb`; la aplicación de escritorio lo ejecuta solo en `127.0.0.1`.

### Variables de entorno

`web/.env.example`, `mac/.env.example` y `android/.env.example` usan las mismas dos líneas. Configura la URL pública con una dirección accesible desde teléfonos y ordenadores. Deja el token vacío en Git.

```text
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=
```

`GATEWAY_PUBLIC_URL` se incluye en los códigos QR. `GATEWAY_TOKEN` se envía en cada `hello`. Si el token del servidor está vacío, cualquiera que acceda a `/ws` puede unirse. Genera un token largo y aleatorio antes de exponer el proceso a Internet:

```bash
openssl rand -hex 32
```

No incluyas el token en commits. Guárdalo en el entorno del proceso o en `web/temp.env` para Docker. `temp.env` no forma parte del repositorio.

### Ejecutar con Node

```bash
cd web
npm install
export GATEWAY_PUBLIC_URL=https://pairspan.example.com
export GATEWAY_TOKEN='paste-the-token'
npm start
```

El proceso escucha en `0.0.0.0:3000` y sirve la página principal, `/p/`, WebSocket `/ws` y `GET /health`.

```bash
curl -fsS http://127.0.0.1:3000/health
```

Un proceso saludable devuelve `{"ok":true,"service":"pairspan",...}`.

### Ejecutar con Docker

Desde `web/`:

```bash
docker build -t pairspan:latest .
```

Crea `web/temp.env`:

```text
NODE_ENV=production
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=paste-the-token
```

[`web/docker-compose.yml`](web/docker-compose.yml) lee ese archivo y publica el puerto `3000` del contenedor en `127.0.0.1:16200`.

```bash
docker compose up -d
curl -fsS http://127.0.0.1:16200/health
```

Coloca un proxy inverso con TLS delante de `127.0.0.1:16200`. Reenvía las actualizaciones WebSocket en `/ws`. Comprueba también la URL pública:

```bash
curl -fsS https://pairspan.example.com/health
```

### Configurar las aplicaciones para tu pasarela

Escribe la misma URL y el mismo token en `mac/.env.example` y `android/.env.example`, y vuelve a compilar. La aplicación de escritorio lee `.env.example` al iniciarse; Android incorpora los valores al APK durante la compilación.

```bash
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

Se rechaza cualquier cliente compilado con otro token. Antes de escanear, el escritorio debe mostrar **Connected to the Pairspan service**. Sigue después los [pasos de emparejamiento](#pair).

## 6. Usar el relé

Cada teléfono recibe un puerto local propio en el ordenador, a partir de `127.0.0.1:44755`. La ventana de escritorio muestra el destino de `adb connect` para cada teléfono.

```bash
adb devices
adb -s 127.0.0.1:44755 shell id
```

Si `adb devices` muestra `authorizing`, desbloquea el teléfono, acepta la solicitud de depuración USB y marca **Permitir siempre**.

Para desconectar un teléfono, pulsa **Remove connection** en Android. Para desconectarlos todos, desactiva el interruptor del escritorio.

## Licencia

[MIT](LICENSE)

## Seguridad y uso responsable

Usa Pairspan solo con dispositivos propios o para los que tengas permiso explícito, con fines legítimos de desarrollo, depuración y administración. No uses ADB para acceso no autorizado, vigilancia oculta, robo de datos, malware ni para eludir controles de seguridad.

En la configuración USB descrita arriba, el propietario debe activar la depuración USB, desbloquear el teléfono y aceptar la autorización de depuración de Android. Autoriza solo ordenadores de confianza. El emparejamiento y la reconexión automática deben respetar esa autorización y no eludir permisos revocados. Consulta la [documentación de ADB de Android](https://developer.android.com/tools/adb).

Mantén privados los tokens de emparejamiento y las claves ADB. Para finalizar el acceso, elimina la conexión de Pairspan y revoca las autorizaciones de depuración USB en las opciones de desarrollador de Android; desactiva la depuración cuando ya no la necesites. Respeta el modelo de seguridad y permisos de Android y la [política de Google Play sobre abuso de dispositivos y redes](https://support.google.com/googleplay/android-developer/answer/16559646) aplicable. La autorización USB por sí sola no demuestra el cumplimiento de las políticas de Google Play.
