# Get started with UUOB

It takes about five minutes:

1. Install **UUOB** on your devices.
2. **Link** them with a short code.
3. Install the **apps** you want (Tasks, Calendar, Photos).

Downloads for everything: **[github.com/jachymri/uuob-releases](https://github.com/jachymri/uuob-releases)**

> **Tip:** UUOB only syncs while it runs. One device that is usually on, such as a Mac, a home server or a
> Raspberry Pi, keeps your devices in sync even when the others are off.

## 1. Install UUOB

### Mac

1. Download **UUOB for Mac** (it works on Apple silicon and Intel Macs).
2. Open the file and drag UUOB into **Applications**.
3. The first time you open it, macOS blocks it, because the app isn't signed with a paid Apple account.
   Go to **System Settings → Privacy & Security** and click **Open Anyway**.
4. UUOB lives in the **menu bar** (top right). To keep it running, turn on
   **Devices → This computer → Start at login**.

With Homebrew instead:

```bash
brew tap jachymri/uuob https://github.com/jachymri/uuob-releases
brew install --cask uuob
```

### Android

1. On the phone, open the download page and tap **UUOB for Android**.
2. When asked, allow your browser to install apps.
3. Open UUOB and allow notifications. It uses them to show your alerts.
4. Optional: turn on **Devices → This phone → Start when the phone turns on**.

With F-Droid instead: add this repository in F-Droid (or Droid-ify), then install UUOB from there. Updates
then come through F-Droid.

```
https://jachymri.github.io/uuob-releases/fdroid/repo?fingerprint=3BE594965991D33F960870832B42EC62690731D45EEB13158B7C2A732A12ABA8
```

### Linux and headless servers (Raspberry Pi)

On Linux, UUOB runs as a command with no window. That suits a home server or a Raspberry Pi
(Raspberry Pi OS 12, Debian 12 or newer, on ARM or Intel/AMD). It keeps your data and syncs it; the apps
themselves (Tasks, Calendar, Photos) are for Mac and Android.

1. Install it with one line. It asks whether UUOB should start by itself when the computer turns on.

   ```bash
   curl -fsSL https://raw.githubusercontent.com/jachymri/uuob-releases/main/install.sh | sh
   ```

2. Join your network with a code from another device (see step 2 below):

   ```bash
   uuob join 42-amber-river-falcon
   ```

3. Optional, but this is what a server is good at: make it a **storage device**, so it keeps a copy of
   everything.

   ```bash
   uuob keep on
   ```

4. Check that it works:

   ```bash
   uuob status
   ```

To update, run the install line again. `uuob help` lists all commands.

**With Docker instead:**

```bash
git clone https://github.com/UUOB/uuob.git && cd uuob
docker build -f docker/server.Dockerfile -t uuob .
docker run -d --name uuob --restart unless-stopped --network host -v uuob-data:/data uuob
docker exec uuob uuob join 42-amber-river-falcon
```

## 2. Link your devices

Your first device needs nothing: it starts as a network of one. To add another device:

1. On a device that's **already set up**, open **Devices → Add a device**. It shows a code like
   `42-amber-river-falcon` and a QR code.
2. On the **new device**, open UUOB, choose **Join a network**, and type the code or tap **Scan QR code**.
   On a server, run `uuob join <code>`.

That's it. Anything already on the new device is kept and added to the network. Do the same for each
device.

A server can also make the code: run `uuob invite` there, then type that code on your Mac or phone.

## 3. Add apps

1. Install **Tasks**, **Calendar** or **Photos** from the same download page, the same way as UUOB.
2. Open the app. It finds UUOB on the same device and asks to connect.
3. UUOB asks, for example, *"Tasks wants to use your network"*. Tap **Allow**.

Done. The app's data now lives in UUOB and appears on all your devices.

### On a device without UUOB

An app also works on a device where UUOB isn't installed. It then uses UUOB on your other devices.

1. In UUOB on any of your devices, open **Apps → Connect with a code**. On a server, run `uuob app-code`.
2. In the app, choose **Use a code instead** and type the code.

## Extras

- **Share with someone.** Once, link with them: one of you opens **People → Add a person** and the other
  types the code it shows. Then open a list in
  Tasks or a calendar in Calendar and choose **Share**, with *Can view* or *Can edit*.
- **Up next widget (Android).** Long-press the home screen → **Widgets** → **UUOB** → **Up next**. It shows
  what's coming from all your apps on a day timeline.
- **Storage devices.** In **Devices → Storage devices**, turn on the switch for a device with plenty of
  space that is usually on. Apps with large data, like Photos, can then keep less on your phone.
- **AI access.** **AI access → Connect an AI** lets an AI assistant (Claude or another) use your data. It
  starts with no access, and you choose what it may read or change, app by app.
- **Updates.** Each app shows an **Update** button when a new version is out. Every update is signed and
  checked before it installs.

## Something not working?

- **An app says "Waiting for UUOB".** Open UUOB on that device, or turn on a device that has UUOB. Your
  changes are kept and sent when it's back.
- **Devices don't sync.** Both need UUOB running, version 0.2 or newer. Keeping everything up to date
  is easiest.
- **Mac says the app "can't be opened".** See step 3 of [Mac](#mac): **Open Anyway** in Privacy & Security.
