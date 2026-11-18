# Working on UUOB

The principle behind every change: **UUOB itself does almost nothing.** It sits on each device and runs the network (devices, people, allowed apps, synced data), and you open it only to control that network. Everything a person sees and uses comes from the apps they install: their widgets, alerts, notifications and screens. Calendar and Tasks are just the first apps, not part of UUOB, and no one has to install them.

- Never make UUOB require, assume, or build in a particular app.
- What an app shows is posted by the app itself, under its own name and icon. UUOB only hands the data over.
- Help for apps goes through one general mechanism that any app can use, never through code written for one app.
- When a feature could live in UUOB or in an app, put it in the app.

Read the full reasoning in [docs/idea.md](docs/idea.md) ("UUOB itself does almost nothing").
