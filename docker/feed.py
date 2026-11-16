#!/usr/bin/env python3
"""Writes what the public downloads repo holds besides the files themselves.

  feed.py <repo> <app> <Name> <version> <dist>   the app's update feed, <dist>/releases/<app>.json,
                                                 and its Homebrew cask, <dist>/releases/Casks/
  feed.py <repo> --page <checkout>               the downloads page, README.md, from the feeds
(The F-Droid repo is written by docker/fdroid.sh.)

The feed is the one Tauri's updater reads, with Android next to the Mac
(see crates/uuob-update). Called by docker/release.sh.
"""
import datetime
import hashlib
import json
import pathlib
import sys

repo = sys.argv[1]
# The F-Droid repo's signing key (docker/fdroid.sh prints it); F-Droid apps check it.
FDROID_KEY = "3BE594965991D33F960870832B42EC62690731D45EEB13158B7C2A732A12ABA8"
# In the order to install them in: the name, and the one-line description Homebrew shows.
APPS = {
    "uuob": ("UUOB", "Your own backend: links your devices and keeps your apps' data on them"),
    "tasks": ("Tasks", "Tasks, kept in your own UUOB"),
    "calendar": ("Calendar", "Calendar, kept in your own UUOB"),
    "photos": ("Photos", "Photos and videos, kept in your own UUOB"),
}


def feed(app, name, version, dist):
    dist = pathlib.Path(dist)
    base = f"https://github.com/{repo}/releases/download/{app}-v{version}/{name}-{version}"

    def file(suffix):
        return {"url": f"{base}-{suffix}", "signature": (dist / f"{name}-{version}-{suffix}.sig").read_text().strip()}

    mac = file("mac.app.tar.gz")  # one build for Apple silicon and Intel
    # No "name": Tauri's updater takes one for the version, and then can't read the feed.
    out = {
        "version": version,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {"android-aarch64": file("arm64.apk"), "darwin-aarch64": mac, "darwin-x86_64": mac},
    }
    (dist / "releases" / f"{app}.json").write_text(json.dumps(out, indent=2) + "\n")

    # Homebrew: the same repo is a tap, so `brew upgrade` follows the releases too.
    token = "uuob" if app == "uuob" else f"uuob-{app}"
    dmg = hashlib.sha256((dist / f"{name}-{version}-mac.dmg").read_bytes()).hexdigest()
    casks = dist / "releases" / "Casks"
    casks.mkdir(exist_ok=True)
    (casks / f"{token}.rb").write_text(f'''cask "{token}" do
  version "{version}"
  sha256 "{dmg}"

  url "https://github.com/{repo}/releases/download/{app}-v#{{version}}/{name}-#{{version}}-mac.dmg"
  name "{name}"
  desc "{APPS[app][1]}"
  homepage "https://github.com/{repo}"

  app "{name}.app"
end
''')


def page(checkout):
    checkout = pathlib.Path(checkout)
    rows = []
    for app, (name, _) in APPS.items():
        path = checkout / f"{app}.json"
        if not path.exists():
            continue
        f = json.loads(path.read_text())
        apk = f["platforms"]["android-aarch64"]["url"]
        dmg = f["platforms"]["darwin-aarch64"]["url"].replace(".app.tar.gz", ".dmg")
        rows.append(f"| **{name}** | {f['version']} | [Download for Android]({apk}) | [Download for Mac]({dmg}) |")
    user, name = repo.split("/")
    fdroid = f"https://{user}.github.io/{name}/fdroid/repo?fingerprint={FDROID_KEY}"
    uuob = json.loads((checkout / "uuob.json").read_text())["version"]
    linux = f"https://github.com/{repo}/releases/download/uuob-v{uuob}/uuob-{uuob}-linux"
    (checkout / "README.md").write_text(
        PAGE.format(rows="\n".join(rows), repo=repo, tap=repo.replace("-releases", ""), fdroid=fdroid, linux=linux)
    )


PAGE = """# UUOB downloads

UUOB is your own backend: it runs on your devices, links them into one
end-to-end encrypted network and keeps your apps' data there. Tasks,
Calendar and Photos are apps that use it.

| App | Version | Android | Mac |
| --- | --- | --- | --- |
{rows}

## Installing

Install **UUOB first**, then the other apps, all on the same device.

On a device that has no UUOB (a tablet, a second phone), install only the apps. In UUOB on one of your
other devices choose **Apps → Connect with a code** and type the code into the app: it then uses your
UUOB on your other devices, whichever of them is on.

- **Android** (arm64 phones): open the link on the phone and allow your browser to install apps when asked.
- **Android with F-Droid** (or another F-Droid app, such as Droid-ify): add this repository, then install
  and update the apps from there.

  ```
  {fdroid}
  ```
- **Mac** (Apple silicon and Intel): open the file and drag the app to Applications. The apps are not signed
  with an Apple developer account, so the first time macOS blocks them: go to System Settings → Privacy &
  Security and choose **Open Anyway**.
- **Mac with Homebrew**: this repo is also a tap.

  ```bash
  brew tap {tap} https://github.com/{repo}
  brew install --cask uuob uuob-tasks uuob-calendar uuob-photos
  ```

  `brew upgrade` then follows new versions. The first-open step above still applies.

## On a server or a Raspberry Pi

UUOB also runs without a window. One line installs it on Linux (Raspberry Pi OS 12 or Debian 12 and newer,
ARM and Intel/AMD) and asks whether it should start by itself when the computer turns on; run the same
line again to update it:

```bash
curl -fsSL https://raw.githubusercontent.com/{repo}/main/install.sh | sh
```

Then join your network and, if this computer should keep a copy of everything, make it a storage device:

```bash
uuob join <code>         # the code from Devices → Add a device on your Mac or phone
uuob keep on
uuob autostart on|off    # start by itself when the computer turns on, or only with `uuob run`
```

`uuob help` lists the other commands. The program by itself: [ARM (Raspberry Pi)]({linux}-arm64),
[Intel/AMD]({linux}-amd64).

## Updating

Each app updates itself: when a newer version is published here, the app shows an **Update** button.
Every update is signed, and the app checks the signature before installing it.

Version 0.3 changes the code that connects an app (words instead of six digits), so update UUOB and the
apps together. Devices on 0.3 and 0.2 sync with each other; devices on 0.1 do not sync with newer ones.
"""

if sys.argv[2] == "--page":
    page(sys.argv[3])
else:
    feed(*sys.argv[2:6])
