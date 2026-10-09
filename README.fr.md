<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">
    <img src="web/assets/pairspan-icon.png" alt="Pairspan" width="100">
  </a>
</p>

<h1 align="center">Pairspan</h1>

<p align="center">Connectez votre téléphone Android à macOS ou Windows via ADB sans fil.</p>

<p align="center"><strong>Associez une fois. Connectez sans fil. Déployez facilement.</strong></p>

<p align="center">
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0"><img src="https://img.shields.io/badge/release-v1.0.0-39795c" alt="Release v1.0.0"></a>
  <img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Android-39795c" alt="Platforms: macOS, Windows and Android">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-39795c" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">Site web</a> ·
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0">Télécharger</a> ·
  <a href="#setup-guide">Guide d’installation</a> ·
  <a href="CONTRIBUTING.md">Contribuer</a>
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
  <strong>Soutenir le projet</strong><br>
  <a href="https://buymeacoffee.com/ferhatozcelik">Buy Me a Coffee</a> ·
  <a href="https://thanks.dev/donations">thanks.dev</a>
</p>

<br>

<table align="center">
  <tr>
    <th align="center">Android · Connecté</th>
    <th align="center">Android · Association</th>
    <th align="center">macOS · Code QR et jeton</th>
  </tr>
  <tr>
    <td align="center" valign="top"><img src="docs/screenshots/android-connected.png" alt="Android · Connecté" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/android-pairing.png" alt="Android · Association" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/macos-pairing.png" alt="macOS · Code QR et jeton" width="300"></td>
  </tr>
</table>

## Télécharger

[Version 1.0.0](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0) : macOS, Windows et Android.

## Dossiers

| Composant | Dossier |
| --- | --- |
| Android | [`android/`](android/) |
| macOS et Windows | [`mac/`](mac/) |
| Web | [`web/`](web/) |

```bash
cd web && npm install && npm start
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

<a id="setup-guide"></a>

## Guide d’installation

Pairspan comprend une application de bureau (macOS ou Windows) et une application Android. Toutes deux se connectent à la passerelle `https://pairspan.ferhatozcelik.com` : le téléphone et l’ordinateur n’ont pas besoin d’être sur le même réseau. Un ordinateur peut accueillir plusieurs téléphones simultanément.

### 1. Installer l’application de bureau

**macOS**

1. Téléchargez `pairspan-mac-1.0.0.dmg` depuis la [page des versions](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0) et glissez Pairspan dans Applications.
2. L’application n’est pas notarisée par Apple. Si macOS bloque le premier lancement, faites un clic droit sur Pairspan et choisissez **Ouvrir**.
3. Pairspan se trouve dans la barre des menus. Cliquez sur son icône pour afficher le jeton d’association et le code QR.
4. `adb` est fourni : aucune installation supplémentaire n’est nécessaire.

**Windows**

1. Téléchargez `pairspan-windows-1.0.0.exe` depuis la même page et exécutez-le.
2. L’installateur n’est pas signé. Si SmartScreen affiche un avertissement, choisissez **Informations complémentaires → Exécuter quand même**.
3. Pairspan se trouve dans la zone de notification. Cliquez sur son icône pour afficher le jeton et le code QR.
4. `adb.exe` est fourni.

Activez ou désactivez Pairspan avec l’interrupteur en haut de la fenêtre ou **Enabled** dans le menu de la zone de notification. Aucun téléphone ne peut se connecter lorsqu’il est désactivé.

### 2. Installer l’application Android

1. Téléchargez et installez l’APK depuis la page des versions. Autorisez les installations depuis votre navigateur ou gestionnaire de fichiers si nécessaire. Android 8.0 ou ultérieur est requis, et Android 11 ou ultérieur pour le débogage sans fil.
2. Ouvrez Pairspan et autorisez les notifications et la caméra. La caméra sert uniquement à scanner le code QR.
3. Dans **Paramètres → À propos du téléphone**, appuyez sept fois sur **Numéro de build**. Dans **Options pour les développeurs**, activez **Débogage USB** et, si disponible, **Débogage sans fil**.

### 3. Accorder l’accès au système (une seule fois)

Pairspan peut activer ses services d’accessibilité et de notifications ainsi qu’ADB sans fil sans passer par les paramètres. Android exige d’abord l’autorisation `WRITE_SECURE_SETTINGS`, accordée depuis un ordinateur. L’application de bureau s’en charge :

1. Branchez le téléphone en USB, avec le débogage USB activé et Pairspan installé.
2. Dans **One-time phone permission**, cliquez sur **Find USB phones**.
3. Si le téléphone demande d’autoriser le débogage USB, acceptez puis relancez la recherche.
4. Cliquez sur **Grant permission** sous votre téléphone. L’application exécute ces commandes avec son `adb` et fait passer ADB au port 5555 pour une connexion sans câble :

```bash
adb shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS
adb tcpip 5555
```

5. Sur le téléphone, appuyez sur **Autoriser** et cochez **Toujours autoriser**. Aucune commande à saisir.

Remarques :

- L’autorisation reste valide jusqu’à la désinstallation de Pairspan. Le réglage du port 5555 dure jusqu’au redémarrage du téléphone ; cliquez ensuite à nouveau sur **Grant permission**.
- En cas d’erreur de sécurité, activez **Débogage USB (paramètres de sécurité)** dans les options pour les développeurs et réessayez (certains Xiaomi/Redmi/POCO).
- Vous pouvez aussi exécuter la commande avec n’importe quel `adb`. Android l’affiche sous **Getting ready → System access**, avec **Copy command**.
- Pour les téléphones sans débogage sans fil, exécutez une fois `adb tcpip 5555` en USB. Pairspan utilisera ce port.

<a id="pair"></a>

### 4. Associer

1. Vérifiez que l’application de bureau est activée et affiche **Connected to the Pairspan service**.
2. Dans Android, appuyez sur **Scan QR code** et scannez le code du bureau ou saisissez le jeton d’association.
3. Le téléphone apparaît sous **Connected devices**, avec son **Device ID** et le **Client ID** de l’ordinateur. Ces identifiants sont aussi dans **Settings** sur Android.
4. Pour ajouter un téléphone, répétez l’association avec le nouveau code. Un code utilisé est remplacé immédiatement.

### 5. Auto-hébergement

Les applications de bureau et Android n’exposent pas directement ADB l’une à l’autre. Elles se connectent à la passerelle dans [`web/`](web/). Le service public est `https://pairspan.ferhatozcelik.com`. Pour héberger votre propre service, démarrez ce processus et compilez les deux applications avec la même URL et le même jeton.

### Prérequis

- Node.js 22 ou Docker.
- Un nom d’hôte avec TLS si la passerelle est accessible depuis Internet. Les clients utilisent `https://` et WebSocket `wss://…/ws`.
- Le même `GATEWAY_TOKEN` pour le serveur et les deux applications.

Les sessions sont conservées en mémoire et disparaissent au redémarrage du processus. La passerelle n’exécute pas `adb` ; l’application de bureau le fait uniquement sur `127.0.0.1`.

### Variables d’environnement

`web/.env.example`, `mac/.env.example` et `android/.env.example` utilisent les deux mêmes lignes. Choisissez une URL publique accessible aux téléphones et aux ordinateurs. Laissez le jeton vide dans Git.

```text
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=
```

`GATEWAY_PUBLIC_URL` est intégré aux codes QR. `GATEWAY_TOKEN` est envoyé avec chaque `hello`. Si le jeton du serveur est vide, toute personne pouvant atteindre `/ws` peut rejoindre le service. Générez un long jeton aléatoire avant de rendre le processus accessible depuis Internet :

```bash
openssl rand -hex 32
```

Ne commitez pas le jeton. Placez-le dans l’environnement du processus ou dans `web/temp.env` pour Docker. `temp.env` ne fait pas partie du dépôt.

### Exécuter avec Node

```bash
cd web
npm install
export GATEWAY_PUBLIC_URL=https://pairspan.example.com
export GATEWAY_TOKEN='paste-the-token'
npm start
```

Le processus écoute sur `0.0.0.0:3000` et sert la page d’accueil, `/p/`, WebSocket `/ws` et `GET /health`.

```bash
curl -fsS http://127.0.0.1:3000/health
```

Un processus sain renvoie `{"ok":true,"service":"pairspan",...}`.

### Exécuter avec Docker

Depuis `web/` :

```bash
docker build -t pairspan:latest .
```

Créez `web/temp.env` :

```text
NODE_ENV=production
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=paste-the-token
```

[`web/docker-compose.yml`](web/docker-compose.yml) lit ce fichier et publie le port `3000` du conteneur sur `127.0.0.1:16200`.

```bash
docker compose up -d
curl -fsS http://127.0.0.1:16200/health
```

Placez un proxy inverse TLS devant `127.0.0.1:16200`. Transmettez les demandes de mise à niveau WebSocket sur `/ws`. Vérifiez aussi l’URL publique :

```bash
curl -fsS https://pairspan.example.com/health
```

### Configurer les applications pour votre passerelle

Écrivez la même URL et le même jeton dans `mac/.env.example` et `android/.env.example`, puis recompilez. L’application de bureau lit `.env.example` au démarrage ; Android intègre les deux valeurs à l’APK lors de la compilation.

```bash
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

Un client compilé avec un jeton différent est rejeté. Avant de scanner, le bureau doit afficher **Connected to the Pairspan service**. Suivez ensuite les [étapes d’association](#pair).

## 6. Utiliser le relais

Chaque téléphone reçoit son propre port local sur l’ordinateur, à partir de `127.0.0.1:44755`. La fenêtre de bureau affiche la cible `adb connect` de chaque téléphone.

```bash
adb devices
adb -s 127.0.0.1:44755 shell id
```

Si `adb devices` affiche `authorizing`, déverrouillez le téléphone, acceptez la demande de débogage USB et cochez **Toujours autoriser**.

Pour déconnecter un téléphone, appuyez sur **Remove connection** dans Android. Désactivez l’interrupteur du bureau pour tout déconnecter.

## Licence

[MIT](LICENSE)

## Sécurité et utilisation responsable

Utilisez Pairspan uniquement sur vos propres appareils ou avec une autorisation explicite, à des fins légitimes de développement, de débogage et de gestion. N’utilisez pas ADB pour un accès non autorisé, une surveillance dissimulée, le vol de données, des logiciels malveillants ou le contournement de protections.

Lors de la configuration USB décrite ci-dessus, le propriétaire doit activer le débogage USB, déverrouiller le téléphone et accepter l’autorisation de débogage d’Android. N’autorisez que des ordinateurs de confiance. L’association et la reconnexion automatique doivent respecter cette autorisation et ne pas contourner les permissions révoquées. Consultez la [documentation ADB d’Android](https://developer.android.com/tools/adb).

Gardez les jetons d’association et les clés ADB privés. Pour mettre fin à l’accès, supprimez la connexion Pairspan et révoquez les autorisations de débogage USB dans les options pour les développeurs d’Android ; désactivez le débogage lorsqu’il n’est plus nécessaire. Respectez le modèle de sécurité et de permissions d’Android et la [règle Google Play relative à l’utilisation abusive des appareils et des réseaux](https://support.google.com/googleplay/android-developer/answer/16559646) applicable. L’autorisation USB seule ne prouve pas la conformité aux règles Google Play.
