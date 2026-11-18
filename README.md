# UUOB

**Your data lives on your devices. Apps just plug in.**

UUOB (Universal User-Owned Backend) links your own devices, like your Mac, your phone or a home server,
into one private network. Apps keep their data there instead of on some company's servers.

**[Get started](docs/setup.md)** · **[Downloads](https://github.com/jachymri/uuob-releases)**

## The problem

Every app today has its own servers and its own account, and it keeps your data in its own cloud. Your
life ends up scattered across dozens of services. You can't easily move it, and you can't connect it.
If a service shuts down or changes its rules, your data goes with it.

Self-hosting fixes that, but it usually means servers, domains, passwords and updates to look after.

## What UUOB does differently

- **Your devices are the backend.** Data is stored only on your own devices. No accounts, and nothing is
  kept in anyone's cloud.
- **Apps don't own your data.** An app is just a window onto it. Switch to another app and your data
  stays where it is.
- **Private by design.** Everything is end-to-end encrypted: only your devices, and people you share
  something with, can read it. When devices
  can't reach each other directly, a public relay passes the data along, but it sees only scrambled
  bytes.
- **Simple, like a normal app.** No server setup. You link devices by typing a short code like
  `42-amber-river-falcon`.
- **Apps work together, with your OK.** Each app sees only its own data. Calendar can show your tasks,
  but only after you allow it.

## How it works

1. **Install UUOB** on your devices. It runs quietly in the background.
2. **Link them** with a code. They now sync directly with each other.
3. **Add apps.** Each app asks UUOB to connect, you tap *Allow*, and its data appears on all your
   devices.

You can also **share** a single list or calendar with someone who uses UUOB, and let an **AI assistant**
see exactly the data you choose, app by app.

[Step-by-step setup for Mac, Android, Linux and servers →](docs/setup.md)

## Apps for UUOB

| App | What it is |
| --- | --- |
| **[Tasks](https://github.com/UUOB/tasks)** | To-do lists with dates, alerts and sharing |
| **[Calendar](https://github.com/UUOB/calendar)** | Your calendars, plus any calendar link you subscribe to (Google, holidays) |
| **[Photos](https://github.com/UUOB/photos)** | Photos and videos, kept on your devices instead of a cloud |

Their alerts ring on your phones and computers, also while the app is closed: UUOB keeps the time,
and on Android the alert is the app's own notification. On Android, Calendar has an **Up next** widget
with today and tomorrow.

## Runs on

- **Mac** (Apple silicon and Intel)
- **Android** phones
- **Linux**, including servers (it runs without a window)

## Learn more

- [The idea behind UUOB](docs/idea.md): why the relationship between apps and your data should be the
  other way round.
- [For developers](docs/developers.md): how it works inside, building it, and making your own app.

## License

[GNU Affero General Public License, version 3 or later](LICENSE). You may use, change and share UUOB and
these apps; if you distribute a changed version or run one as a service for others, you publish your
source under the same license.
