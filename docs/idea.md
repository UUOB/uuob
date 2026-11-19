# The idea behind UUOB

Today, most digital applications operate within their own isolated ecosystems. Each service maintains its own infrastructure, stores its own user data, and controls how that data can be accessed, shared, or transferred. As a result, users become dependent on individual providers, while their personal information is fragmented across numerous platforms.

**UUOB proposes reversing this relationship: instead of applications owning the infrastructure that holds user data, users own their infrastructure, and applications connect to it.**

The idea is to create an open, universal foundation that separates personal data from the applications that use it. Users would maintain control over their information, independently of any particular service, while remaining free to choose which applications interact with it.

This approach aims to address several fundamental problems with today's software ecosystem: vendor lock-in, fragmented personal information, duplicated infrastructure, limited interoperability, and the complexity of managing self-hosted alternatives.

A central ambition is to make data ownership and independence convenient rather than burdensome. Privacy, openness, and decentralization should not require sacrificing the simplicity and seamless experience people expect from modern software.

UUOB is therefore not intended to be another isolated application or replacement for a specific service. It is a proposal for a different relationship between users, their data, and the software they use.

**The ultimate vision is a digital ecosystem where applications are interchangeable, personal information remains under the user's control, and switching software no longer means surrendering ownership or starting over.**

## UUOB itself does almost nothing

The UUOB application just sits on each device and keeps the network running: your devices, the people you share with, the apps you allow, and their data synced between them. You open it to control that network, and that's all it is for.

Everything else you want from the system comes from the apps you choose to install. Calendar and Tasks exist only because there were no apps yet. They are examples, not part of UUOB. Someone else might use a different calendar, or write their own, and UUOB has to work just as well for them.

So UUOB stays general and assumes nothing about which apps you have:

- **No app is required.** UUOB never expects Calendar, Tasks or any other app to be installed, and it works fully without them.
- **No app is built in.** UUOB has no screens, widgets or features that belong to one app. What an app shows (its widgets, its alerts, its notifications) is the app's own, under its own name and icon.
- **The same rules for every app.** Where UUOB helps apps with something they can't do alone (keeping time for alerts while the app is closed, learning of changes made on other devices), it does it through one standard way that any app can use, not through code written for a particular app.

When a feature could live in UUOB or in an app, it belongs in the app. UUOB is the backbone, and the apps are the face.

## UUOB guards the data; allowing an app means trusting it

UUOB's job is to guard your data. Every app sees only what you allowed: its own data, plus anything you agreed to share with it. That is the whole security model, and it is the same for everything that touches your data:

- **Apps.** Allowing an app means trusting it with that data. It doesn't matter who built the app: an app from someone else gets exactly the same as one you wrote, once you allow it. UUOB checks that it hands data only to the app you allowed, and not to another one that took its name.
- **AI assistants.** They start with nothing and see only the apps you open to them, read or read and write.
- **People.** They see only the lists and calendars you share with them, and nothing about the rest.

A device that can't run UUOB itself, like an iPhone, doesn't change this. Its apps reach your UUOB on another device that is usually on, such as a home server, through a code you gave them, and get the same guarded view.
