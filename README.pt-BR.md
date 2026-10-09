<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">
    <img src="web/assets/pairspan-icon.png" alt="Pairspan" width="100">
  </a>
</p>

<h1 align="center">Pairspan</h1>

<p align="center">Conecte seu celular Android ao macOS ou Windows por ADB sem fio.</p>

<p align="center"><strong>Pareie uma vez. Conecte sem fios. Implante com facilidade.</strong></p>

<p align="center">
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0"><img src="https://img.shields.io/badge/release-v1.0.0-39795c" alt="Release v1.0.0"></a>
  <img src="https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Android-39795c" alt="Platforms: macOS, Windows and Android">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-39795c" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://pairspan.ferhatozcelik.com">Site</a> ·
  <a href="https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0">Baixar</a> ·
  <a href="#setup-guide">Guia de configuração</a> ·
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

<br>

<table align="center">
  <tr>
    <th align="center">Android · Conectado</th>
    <th align="center">Android · Pareamento</th>
    <th align="center">macOS · QR code e token</th>
  </tr>
  <tr>
    <td align="center" valign="top"><img src="docs/screenshots/android-connected.png" alt="Android · Conectado" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/android-pairing.png" alt="Android · Pareamento" width="200"></td>
    <td align="center" valign="top"><img src="docs/screenshots/macos-pairing.png" alt="macOS · QR code e token" width="300"></td>
  </tr>
</table>

## Baixar

[Versão 1.0.0](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0): macOS, Windows e Android.

## Pastas

| Componente | Pasta |
| --- | --- |
| Android | [`android/`](android/) |
| macOS e Windows | [`mac/`](mac/) |
| Web | [`web/`](web/) |

```bash
cd web && npm install && npm start
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

<a id="setup-guide"></a>

## Guia de configuração

O Pairspan tem um aplicativo para computador (macOS ou Windows) e um para Android. Ambos se conectam ao gateway em `https://pairspan.ferhatozcelik.com`, então o celular e o computador não precisam estar na mesma rede. Um computador pode conectar vários celulares ao mesmo tempo.

### 1. Instale o aplicativo para computador

**macOS**

1. Baixe `pairspan-mac-1.0.0.dmg` na [página de versões](https://github.com/ferhatozcelik/pairspan/releases/tag/v1.0.0) e arraste o Pairspan para Aplicativos.
2. O aplicativo não tem notarização da Apple. Se o macOS bloquear a primeira abertura, clique com o botão direito no Pairspan e escolha **Abrir**.
3. O Pairspan fica na barra de menus. Clique no ícone para ver o token de pareamento e o QR code.
4. O `adb` está incluído; não é necessário instalar mais nada.

**Windows**

1. Baixe `pairspan-windows-1.0.0.exe` na mesma página e execute-o.
2. O instalador não tem assinatura de código. Se o SmartScreen avisar, escolha **Mais informações → Executar assim mesmo**.
3. O Pairspan fica na bandeja do sistema. Clique no ícone para ver o token e o QR code.
4. O `adb.exe` está incluído.

Use o interruptor no topo da janela ou **Enabled** no menu da bandeja para ativar ou desativar o Pairspan. Enquanto estiver desativado, nenhum celular poderá se conectar.

### 2. Instale o aplicativo Android

1. Baixe e instale o APK da página de versões. Se solicitado, permita instalações pelo navegador ou gerenciador de arquivos. É necessário Android 8.0 ou mais recente; para depuração sem fio, Android 11 ou mais recente.
2. Abra o Pairspan e permita notificações e acesso à câmera. A câmera é usada apenas para ler o QR code.
3. Em **Configurações → Sobre o telefone**, toque sete vezes em **Número da versão**. Em **Opções do desenvolvedor**, ative **Depuração USB** e, se disponível, **Depuração sem fio**.

### 3. Conceda acesso ao sistema (uma vez)

O Pairspan pode ativar seus serviços de acessibilidade, notificações e ADB sem fio sem abrir as Configurações. Para isso, o Android exige a permissão `WRITE_SECURE_SETTINGS`, concedida por um computador. O aplicativo para computador faz isso por você:

1. Conecte o celular por USB, com a depuração USB ativada e o Pairspan instalado.
2. Em **One-time phone permission**, clique em **Find USB phones**.
3. Se o celular pedir autorização para depuração USB, aceite e procure novamente.
4. Clique em **Grant permission** abaixo do celular. O aplicativo usa seu `adb` incluído para executar estes comandos e mudar o ADB para a porta 5555, permitindo conexão sem cabo:

```bash
adb shell pm grant com.pairspan android.permission.WRITE_SECURE_SETTINGS
adb tcpip 5555
```

5. No celular, toque em **Permitir** e marque **Sempre permitir**. Não é necessário digitar comandos.

Observações:

- A permissão dura até o Pairspan ser desinstalado. A configuração da porta 5555 dura até o celular reiniciar; depois, clique novamente em **Grant permission**.
- Em caso de erro de segurança, ative **Depuração USB (configurações de segurança)** nas opções do desenvolvedor e tente novamente (alguns Xiaomi/Redmi/POCO).
- Você também pode executar o comando com qualquer `adb`. O aplicativo Android o mostra em **Getting ready → System access**, com **Copy command**.
- Em celulares sem depuração sem fio, execute `adb tcpip 5555` uma vez por USB. O Pairspan usará essa porta.

<a id="pair"></a>

### 4. Pareie

1. Verifique se o aplicativo para computador está ativado e mostra **Connected to the Pairspan service**.
2. No Android, toque em **Scan QR code** e leia o código do computador ou digite o token de pareamento.
3. O celular aparece em **Connected devices**, com seu **Device ID** e o **Client ID** do computador. Os dois identificadores também aparecem em **Settings** no Android.
4. Para adicionar outro celular, repita o pareamento com o novo código. Um código usado é substituído imediatamente.

### 5. Hospede seu próprio gateway

Os aplicativos para computador e Android não expõem ADB diretamente um ao outro. Ambos se conectam ao gateway em [`web/`](web/). O endereço público é `https://pairspan.ferhatozcelik.com`. Para hospedar seu próprio serviço, inicie esse processo e compile ambos os aplicativos com a mesma URL e o mesmo token.

### Requisitos

- Node.js 22 ou Docker.
- Um nome de host com TLS se o gateway estiver acessível pela internet. Os clientes usam `https://` e WebSocket `wss://…/ws`.
- O mesmo `GATEWAY_TOKEN` no servidor e nos dois aplicativos.

As sessões ficam na memória e são perdidas ao reiniciar o processo. O gateway não executa `adb`; o aplicativo para computador o executa apenas em `127.0.0.1`.

### Variáveis de ambiente

`web/.env.example`, `mac/.env.example` e `android/.env.example` usam as mesmas duas linhas. Defina a URL pública como um endereço acessível pelos celulares e computadores. Deixe o token vazio no Git.

```text
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=
```

`GATEWAY_PUBLIC_URL` é incluído nos QR codes de pareamento. `GATEWAY_TOKEN` é enviado em cada `hello`. Se o token do servidor estiver vazio, qualquer pessoa que acesse `/ws` poderá entrar. Gere um token longo e aleatório antes de expor o processo à internet:

```bash
openssl rand -hex 32
```

Não inclua o token em commits. Coloque-o no ambiente do processo ou em `web/temp.env` para Docker. `temp.env` não faz parte do repositório.

### Execute com Node

```bash
cd web
npm install
export GATEWAY_PUBLIC_URL=https://pairspan.example.com
export GATEWAY_TOKEN='paste-the-token'
npm start
```

O processo escuta em `0.0.0.0:3000` e serve a página inicial, `/p/`, WebSocket `/ws` e `GET /health`.

```bash
curl -fsS http://127.0.0.1:3000/health
```

Um processo saudável retorna `{"ok":true,"service":"pairspan",...}`.

### Execute com Docker

Na pasta `web/`:

```bash
docker build -t pairspan:latest .
```

Crie `web/temp.env`:

```text
NODE_ENV=production
GATEWAY_PUBLIC_URL=https://pairspan.example.com
GATEWAY_TOKEN=paste-the-token
```

[`web/docker-compose.yml`](web/docker-compose.yml) lê esse arquivo e publica a porta `3000` do contêiner em `127.0.0.1:16200`.

```bash
docker compose up -d
curl -fsS http://127.0.0.1:16200/health
```

Coloque um proxy reverso com TLS na frente de `127.0.0.1:16200`. Encaminhe as atualizações de protocolo WebSocket em `/ws`. Confira também a URL pública:

```bash
curl -fsS https://pairspan.example.com/health
```

### Configure os aplicativos para seu gateway

Escreva a mesma URL e o mesmo token em `mac/.env.example` e `android/.env.example` e compile novamente. O aplicativo para computador lê `.env.example` ao iniciar; o Android incorpora os valores ao APK durante a compilação.

```bash
cd mac && npm install && npm start
cd android && ./gradlew :app:assembleDebug
```

Um cliente compilado com outro token é rejeitado. Antes de ler o QR code, o computador deve mostrar **Connected to the Pairspan service**. Depois, siga os [passos de pareamento](#pair).

## 6. Use o relay

Cada celular recebe uma porta local própria no computador, começando em `127.0.0.1:44755`. A janela do computador mostra o destino de `adb connect` para cada celular.

```bash
adb devices
adb -s 127.0.0.1:44755 shell id
```

Se `adb devices` mostrar `authorizing`, desbloqueie o celular, aceite a solicitação de depuração USB e marque **Sempre permitir**.

Para desconectar um celular, toque em **Remove connection** no Android. Para desconectar todos, desligue o interruptor do computador.

## Licença

[MIT](LICENSE)
