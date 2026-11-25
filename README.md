# UUOB

**The backend that runs on your own devices.**

UUOB stores your apps' data on your own devices, syncs it between them and the people you share it with, and
keeps it end-to-end encrypted. Many apps share one UUOB, and each sees only its own data unless you allow more.

- **No servers.** Your laptop, phone or home server is the backend. Nothing is kept in anyone's cloud.
- **One backend, many apps.** Apps plug into your UUOB instead of each running their own. Switch apps and your
  data stays.
- **Encrypted end to end.** Only your devices, and people you share with, can read your data. Relays that help
  devices connect only ever see scrambled bytes.
- **Default-deny.** Each app sees only its own data. Anything more needs your OK.
- **Sharing built in.** Share one list or calendar with someone, as view-only or editable.
- **Made for AI.** Give an assistant exactly the apps you choose, read-only or read and write.
- **Works offline.** Apps keep working without a connection and catch up later.
- **Open source** (AGPL-3.0). Runs on Mac, Android and Linux, including servers.

**[Download](https://github.com/jachymri/uuob-releases)** · **[Get started](docs/setup.md)**

## What UUOB stores

Each app gets one or more **containers**. A container is a single [Automerge](https://automerge.org) document,
synced and end-to-end encrypted, and it holds exactly two kinds of data:

- **Records:** JSON objects grouped into named collections, each with a string id of up to 128 characters.
  Values are any JSON: strings, numbers, booleans, null, arrays and nested objects. When two devices edit the
  same record at the same time, the top-level fields merge one by one. A nested object or array is replaced as
  a whole.
- **Files:** opaque bytes of any size and type, such as a photo, a video or a PDF, stored with their content
  type. Each file is encrypted with its own key in 64 KiB pieces and identified by the SHA-256 of its encrypted
  bytes. An app links a file to its data by keeping the file's id in a record.

That's the whole storage model. There are no schemas, queries or server-side code, and there's no other data
type. Permissions work per container: an app reads and writes its own containers, and a container can be shared
with other people or opened to other apps and AIs only with the user's OK.

## What it's for

| ✅ Good fit | ❌ Not a fit |
| --- | --- |
| Data that belongs to the people using the app | One big dataset everyone sees |
| Used on a few devices, shared with a few people | Social networks, marketplaces, public search |
| Calendars, tasks, notes, photos, journals, budgets, recipes, a family list, a small team's board | Apps that need code running on a server: payments, sending email, heavy processing |

## For people using apps

1. **Install UUOB** on your devices. It runs quietly in the background.
2. **Link them** with a code like `42-amber-river-falcon`.
3. **Open an app.** It asks to connect, you tap *Allow*, and its data is on all your devices.

## For people building apps

Skip the backend: no server, no database, no accounts, no sync code. Your app talks to UUOB through a small SDK:

```js
import { uuob } from './uuob/ui/uuob.js';

uuob.init({ id: 'recipes', name: 'Recipes' });
const recipes = uuob.collection('recipes');

await recipes.put({ id: 'pancakes', title: 'Pancakes', serves: 4 });
recipes.subscribe((all) => render(all));  // also fires for changes made on other devices
```

- **Records and files:** `put / get / delete / list / subscribe`, plus files of any size.
- **Sharing:** keep a list in a container of its own, and the user can share it.
- **Start from a template:** copy [Tasks](https://github.com/UUOB/tasks), [Calendar](https://github.com/UUOB/calendar)
  or [Photos](https://github.com/UUOB/photos).
- **The catch:** your users need UUOB too.

[Developer docs →](docs/developers.md)

## Why I built this

I wanted an AI to look after my calendar, tasks and mail together. But my calendar was split between Apple on
my Mac and Google on my phone. Moving to GrapheneOS made it messier, and Nextcloud freed the data but only
worked with apps I didn't want to use. So I wanted one backend I own, which any app I like can plug into, and
so can any AI I trust.

## Learn more

- [The idea behind UUOB](docs/idea.md): why apps should plug into your data, not own it
- [Setup](docs/setup.md): Mac, Android, Linux and servers
- [For developers](docs/developers.md): how it works inside, written for agents too

## License

[AGPL-3.0 or later](LICENSE)
