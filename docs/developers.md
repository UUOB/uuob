# UUOB for developers

How UUOB works inside, how the code is laid out, and how to build, test and publish it.
For using UUOB, see the [README](../README.md) and [Get started](setup.md).

## How it works

- **UUOB is the backend app.** Install it once per device. It runs in the background (menu bar icon on a Mac, a quiet notification on Android, no window on a server) and holds your data. A new device starts as its own one-device network.
- **One core, native screens.** Everything UUOB does is Rust (`crates/`). Its screens are native to each system: SwiftUI on the Mac (`mac/`), Jetpack Compose on Android (`android/`), GTK and Libadwaita on Linux (`linux/`). They reach the core through a few functions (`crates/uuob-app/src/ffi.rs`, turned into Swift and Kotlin by UniFFI): start UUOB, call a method, wait for the next change. Every method is the same one apps and AI clients use (`Node::call`), so permissions are checked in one place.
- **Linking devices:** on a device that's already set up, go to *Devices → Add a device*. It shows a code like `42-amber-river-falcon` (and a QR code of it). On the new device, choose *Join a network* and type the code. No accounts, no servers to run, no Tailscale.
- **Sync:** devices connect to each other directly (iroh/QUIC, end-to-end encrypted between device keys). Public relays are only used to get through firewalls, and they only see ciphertext. Data is stored only on your devices, encrypted at rest.
- **Apps are separate programs with no backend.** Tasks and Calendar are their own installable apps. On first launch they find UUOB on the same device and ask to connect: UUOB pops up *"Tasks wants to use your network"* and you tap *Allow*. Or, in UUOB, choose *Apps → Connect with a code* and type the code into the app. The app then gets a key and its own *container*, and stores records through a tiny SDK (`ui/uuob.js`): `put / get / delete / list / subscribe`. While UUOB is not reachable the app keeps working on what it last saw: changes to records wait in the app and are sent, field by field, when UUOB is back.
- **Apps on a device without UUOB.** UUOB does not have to be on the device an app runs on (an iPhone cannot keep one running). Type a code from *Apps → Connect with a code* (`uuob app-code` on a server) into the app on that device: the app then reaches your UUOB on your other devices through the same encrypted device-to-device connection, carrying the unchanged app API, and uses whichever of your devices is on. The code is the permission: the app gets a key for its own data, bound to the app's own identity, and only apps linked this way can reach a UUOB from outside the device, and only its app routes (not AI access, not the owner key). Questions an app raises ("Calendar wants to see your tasks", *Share*) appear in UUOB on all your devices, so a server without a screen can serve apps too. UUOB lists each such copy under its app with the name of its device (*On devices without UUOB*), and each can be disconnected by itself, e.g. after losing that device. See `crates/uuob-core/src/remote.rs`.
- **Sharing with other people.** A person is someone else's UUOB (all their devices). Link once under *People* with the same kind of code; that shares nothing. Then share a single list or calendar from its app (*Share*), as *Can view* or *Can edit*. The other person accepts it in their UUOB and sees it in their own Tasks or Calendar. For this, an app keeps each list or calendar in a container of its own (`uuob.containers`), and its first container stays private. A shared container syncs between the owner's devices and each person's devices, never between two guests, so the owner's devices enforce who may change it. Stopping removes the other person's copy the next time their devices connect.
- **Files.** Next to its records a container can hold files of any size (photos, video): `uuob.files(container).put(blob)` returns an id to keep in a record, `.url(id)` gives an address for `<img>` or `<video>`. A file is stored once, outside the documents, encrypted in pieces with a key of its own, and named by the hash of its stored bytes, so every device can check its copy and fetch a good one if it is damaged. Devices fetch missing files from each other over the same connections as sync (a large file that is not on the device is read from another one piece by piece, so a video starts at once), and whoever may see a container (including people it is shared with) may see its files.
- **Storage that stays small.** Each container is a document of its own; only recently used ones are in memory, a change is saved as its own small piece, and two devices compare a fingerprint per document before loading anything. One container still loads whole when used, so an app with a lot of data splits it into many containers.
- **Storage devices.** By default every device keeps everything. Under *Devices* you can make a device with plenty of space a *storage device*: it keeps everything, whatever apps ask. An app can then keep less on the device it runs on (`uuob.containers.keep(id, 'list' | 'none')`): only a container's records, with files fetched when opened and cached, or nothing of it. A device lets go of its copy only after as many storage devices as you chose (*Copies*) have told it, over a live connection, that they hold the same data and every file. With no storage device nothing is ever let go.
- **Photos** is a third app and the stress test for all this: one container per month, a small preview inside each record so the grid works offline, the original as a file, and list-only months on phones.
- **Permissions are default-deny.** An app sees only its own containers. Access to another app's data needs your OK (e.g. Calendar asking to show your tasks). Disconnecting an app in UUOB asks whether to keep or delete its data; deleting removes it from every device. (Uninstalling an app from the OS doesn't tell UUOB, so its data stays until you disconnect it.)
- **AI access (MCP):** *AI access → Connect an AI* creates a key and a ready-to-paste config for Claude Code or any MCP client. Each AI starts with no access; you pick *No access / Read / Read & write* per app.
- **Local server:** `127.0.0.1:7745`, reachable only from the same device (and, for its app routes, by apps you linked from another device). `/api` for apps (see `crates/uuob-api/src/apps.rs`), `/mcp` for AI clients.

## Layout

```
crates/uuob-core   identity, encrypted store, containers, files, permissions, app connections, sync, pairing, people and sharing (+ tests)
crates/uuob-api    local server: app API + MCP (localhost, token-scoped)
crates/uuob-cli    `uuob`: UUOB without a window, and control of a running UUOB from the terminal
crates/uuob-sdk    the native half of the app SDK: where UUOB is, and linking to one on another device
crates/uuob-app    the UUOB app's Rust half: the node and local server, alerts, start at login, updates, and the functions the screens use
crates/uuob-update in-app updates for the apps (Tauri), sharing UUOB's feed format
mac/               UUOB for the Mac (SwiftUI): devices, people, apps, AI access, the menu bar icon
android/           UUOB for Android (Jetpack Compose, Material 3): the same screens, the background service, alerts
linux/             UUOB's window on Linux (Relm4: GTK 4 + Libadwaita)
ui/                shared design system (tokens, icons), the app SDK and the connect screen
docker/            dev + Android images and scripts (everything builds and runs in Docker)
```

The apps have repositories of their own, each with this one as its `uuob` folder (a git submodule), which
gives them the SDK, the connect screen, in-app updates and the build scripts below:
[Tasks](https://github.com/UUOB/tasks), [Calendar](https://github.com/UUOB/calendar) and
[Photos](https://github.com/UUOB/photos). An app of yours can start as a copy of one of them.

## Develop

Everything runs in the dev container (`docker/dev.sh` builds it on first use):

```bash
docker/dev.sh cargo test --workspace                        # core + API tests
docker/dev.sh cargo test -p uuob-core -- --ignored internet # real relays/discovery
docker/dev.sh sh -c 'cargo build -p uuob-cli && node ui/uuob.test.mjs'   # the app SDK against a real UUOB
docker/dev.sh sh -c 'cargo build --release -p uuob-cli && UUOB_BIN=target/release/uuob node docker/stress.mjs'   # 2,000 pictures and a 1 GB video on three devices
docker/dev.sh cargo test -p uuob-app                        # the functions the native screens use
```

Screenshots of UUOB's Linux window run in the Linux image (see *Linux window* below):

```bash
L='docker run --rm -v "$PWD":/work -w /work -v uuob-target-linux:/work/target uuob-linux:1'
$L sh -c 'cargo run -p uuob-core --example seed -- target/demo && docker/shoot.sh target/demo shots/demo 1280x820 dark'
$L docker/shoot-photos.sh shots/photos      # UUOB and Photos side by side (needs Photos built from its repository, see the script)
```

### macOS

The Mac app can't be built on Linux, so GitHub builds it (`.github/workflows/mac.yml`, the same file in
every app's repository; for UUOB it runs `mac/build.sh`, and takes a screenshot of each screen):

```bash
gh workflow run mac.yml          # then: gh run download <run id> -n mac
```

On a Mac with Xcode, `mac/build.sh` builds `UUOB.app` directly. It needs macOS 14 or newer. It isn't
signed with an Apple developer account, so macOS asks for confirmation the first time it is opened.

### Linux window

```bash
docker build -t uuob-linux:1 -f docker/linux.Dockerfile docker
docker run --rm -v "$PWD":/work -w /work -v uuob-target-linux:/work/target uuob-linux:1 cargo build -p uuob-linux
```

It needs Libadwaita 1.5 or newer (Debian 13, Ubuntu 24.04, Fedora 40), so plain `cargo build` leaves it
out. It keeps its data where the `uuob` command finds it (`~/.local/share/app.uuob`).

### Android

```bash
docker/android.sh docker/apk.sh x86_64        # an emulator build, signed, in dist/
docker/emulator.sh start                       # headless emulator (KVM)
docker exec uuob-emu adb install -r dist/UUOB-<version>-x86_64.apk
docker/emulator.sh shot shots/phone.png
```

`android/build.sh` builds the core for the phone, writes its Kotlin bindings and runs Gradle;
`docker/android.sh docker/apk.sh` runs it and leaves a signed arm64 build for real phones in `dist/` (signed with the persisted key in the `uuob-android-keys` volume, so updates install over each other). To try sync from the command line, run a headless peer: `cargo run -p uuob-core --example peer -- <dir> invite`, then enter the printed code on the phone.

### Starting by itself

UUOB only helps while it runs, so each kind of device can start it for you,
and you choose:

- **Computers:** *Devices → This computer → Start at login* starts UUOB in the
  background (tray / menu bar, no window) when you log in. It is a LaunchAgent
  on a Mac and an autostart entry on Linux, both pointing at the app with
  `--hidden` (`autostart` in `crates/uuob-app/src/lib.rs`).
- **Servers:** `uuob autostart on|off` (below).
- **Phones:** *Devices → This phone → Start when the phone turns on* starts
  UUOB in the background (just its notification) once the phone is unlocked
  for the first time after a restart. Left off, UUOB starts when you open it
  or one of its apps, and then stays in the background. Either way it starts
  again after an update, which closes it (`UpdatedReceiver`). The switch turns
  `BootReceiver` on or off; it starts `UuobService`, which starts the node.

### Without a window (servers, the Raspberry Pi)

`uuob` (`crates/uuob-cli`) is UUOB as a command: `uuob run` is a whole device
without a window, and every other command controls the UUOB running on the
same computer, whether started by `uuob run` or by the desktop app. It talks
to the local server with the owner key in UUOB's data folder (`owner.key`,
readable only by you).

On a Linux server or a Raspberry Pi, one line installs the published program
(`docker/install.sh`; run it again to update). It asks whether UUOB should
start by itself when the computer turns on; `uuob autostart on|off` changes
that later (a systemd service running `uuob run` as you):

```bash
curl -fsSL https://raw.githubusercontent.com/jachymri/uuob-releases/main/install.sh | sh
uuob join 42-amber-river-falcon   # the code from Devices → Add a device
uuob keep on                      # make it a storage device
uuob status
```

Or from this source, as a container:

```bash
docker build -f docker/server.Dockerfile -t uuob .
docker run -d --name uuob --restart unless-stopped --network host -v uuob-data:/data uuob
docker exec uuob uuob join 42-amber-river-falcon
```

`uuob help` lists the rest (`devices`, `invite`, `copies`, `people`, `apps`,
`asks`, `allow`, and `call <method> <json>` for anything the window can do).

### Updates

Each app updates itself; UUOB is not involved. An app says where its updates
are published in its own `tauri.conf.json` (`plugins.updater`: an address and
the public key they are signed with), so another author publishes and signs
their own. Tasks, Calendar and Photos share `crates/uuob-update` and the button in
`ui/Update.svelte`: on computers it is Tauri's updater, on Android the same
feed and signatures with the system installer.

UUOB updates itself the same way, from the same feed format and key
(`crates/uuob-app/src/update.rs`): on a Mac the signed `.app.tar.gz` replaces
`UUOB.app`, on Android the signed APK goes to the system installer. Its
version is in `crates/uuob-app/Cargo.toml`; the Mac and Android builds read it
from there.

To publish a new version, raise the version (UUOB: `crates/uuob-app/Cargo.toml`;
an app: `app/src-tauri/tauri.conf.json`), commit, push, and run:

```bash
docker/release.sh          # in an app's repository: uuob/docker/release.sh
```

It builds the APK here and the Mac app on GitHub, signs both with the update
key (kept only in the `uuob-android-keys` volume) and uploads them to the
public downloads repo, where each app has its feed (`tasks.json`). Installed
apps then show "Update to …". The same release also refreshes a Homebrew tap
(`Casks/` in that repo) and an F-Droid repo (`docker/fdroid.sh`, on its
`gh-pages` branch), for people who prefer those.

