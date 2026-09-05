# MeglaNote (Tauri build)

Same markdown notes app, rebuilt on Tauri instead of Electron. Tauri uses
your Mac's built-in WebKit engine rather than shipping its own Chromium
binary — no large unsigned binary for macOS's malware scanner to (wrongly)
flag, which is what kept breaking the Electron version.

## Features

- Markdown editing with live preview, `[[wiki-style linking]]` with
  autocomplete (type `[[` to get a suggestion list), and backlinks
- **Tags live in their own section at the bottom of a note** — not mixed
  into your writing — click a tag (or a sidebar chip) to filter
- Keyboard shortcuts: **⌘N** new note, **⌘F** search, **⌘B** bold,
  **⌘I** italic, **Esc** close autocomplete / go back on narrow windows
- Deleting a note moves it to **Trash** (sidebar link) instead of
  destroying it — restore it, or leave it and it's **automatically
  cleared out after 14 days**
- Collapsible headings — click the arrow next to any `#`/`##`/`###`
  heading in Preview to fold everything under it
- Column breaks — while editing, **Column break** starts a new
  side-by-side column, **Sub-column** splits the current column further
  into side-by-side sub-columns, and **1-column section** drops back to
  normal single-column flow (useful between two column layouts, or to end
  one for good). You can also type `%%col%%`, `%%subcol%%`, `%%endcol%%`
  directly if you'd rather not use the buttons.
- **Quote** button formats the selected line(s) as a styled blockquote —
  a colored left bar, tinted background, and italic text — or just type
  `> ` yourself
- **Star/pin notes** — click the star on a note card, or the Pin button
  in the toolbar — pinned notes stay at the top of the list
- **Drag or paste an image** directly into a note; it's saved into an
  `assets` folder next to your notes and embedded automatically
- **Preferences** (gear icon in the sidebar) — choose where notes are
  saved, including a folder inside iCloud Drive for sync across Macs
- **Native menu bar** — File (New Note), Edit (standard Cut/Copy/Paste/
  Undo/Redo), Window, plus the usual macOS app menu (Hide, Quit, etc.)
- Dark mode toggle (🌙/☀️ button next to the app name), remembered between
  launches — now a **light / dark / pastel / kawaii** cycle (kawaii adds a
  bright candy-pink palette, extra-rounded corners, and a playful rounded
  font), with a matching swatch picker in Preferences for picking a theme
  directly
- **Web Clipper** — a companion browser extension (see the separate
  `meglanote-clipper` download) lets you highlight text on any webpage,
  right-click → "Send to MeglaNote", and it creates a new note with the
  highlighted text, the page title, and a link back to the source
- **Folders** — organize notes into real folders (they show up in Finder
  too). Create one from the sidebar or the toolbar's folder dropdown,
  filter the sidebar by folder, and move a note between folders anytime
- **Color-coded notes** — flag a note with a color (via the toolbar or
  the dot on its card) from a preset palette; it shows as a colored dot
  and a colored left edge on the note card for quick scanning
- Note files are named after the note's title, and rename themselves
  automatically as you edit the title (duplicate titles get " (2)", " (3)",
  etc. appended)

## Where your notes live

`~/Documents/MeglaNote/` by default, one `.md` file per note — named after
the note's title — with a small frontmatter header. Plain text, fully
portable. Change this anytime from **Preferences**; the app's own small
settings file (which just remembers where you pointed it) lives separately
in `~/Library/Application Support/MeglaNote/`, untouched by folder changes.

Note: changing the notes folder in Preferences only points the app at the
new location — it does **not** move your existing notes there for you.
If you switch folders, move the `.md` files (and the `assets` folder, if
you'd used image embedding) over yourself first.

## One-time setup

You'll need two things installed that the Electron version didn't require:

**1. Xcode Command Line Tools** (if you don't already have them):
```bash
xcode-select --install
```

**2. Rust**, via the official installer:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Follow the prompts, then close and reopen Terminal (or run `source
"$HOME/.cargo/env"`) so the `cargo` command is available.

Check both worked:
```bash
cargo --version
```

## Running it

```bash
cd meglanote
npm install
npm run tauri dev
```

The first run will take a few minutes — Cargo compiles the Rust backend
and its dependencies from scratch (this build also pulls in two new
dependencies: `tauri-plugin-deep-link`, for the web clipper's
`meglanote://` URL handoff, and the `url` crate for parsing it).
Every run after that is fast.

**Important for the Web Clipper:** custom URL scheme handlers
(`meglanote://`) are only reliably registered with macOS when running
the properly built `.app` — not `npm run tauri dev`. If you want to test
the clipper, build it (`npm run tauri build`, see below) and launch the
`.app` from Applications at least once before trying the extension.

**If the build fails with an error mentioning `deep_link` or `DeepLink`**
— same situation as the menu bar: written against Tauri's documented
API but not compile-tested here. Paste the error back and it's a quick
fix.

**A note on images:** dropped/pasted images are sent to the Rust backend
as a plain byte array over Tauri's JSON-based IPC, which works fine for
typical screenshots but will feel slow for very large images (many MB) —
not a great fit for huge photo files, but fine for the diagrams and
screenshots a notes app usually deals with.

## Building a standalone .app

```bash
npm run tauri build
```

The finished app lands in `src-tauri/target/release/bundle/macos/` (a
`.app`) and `src-tauri/target/release/bundle/dmg/` (a `.dmg` installer).
Open the `.dmg` (or just find the `.app` directly) and drag **MeglaNote**
into your **Applications** folder — from then on it launches like any
other Mac app, no Terminal needed.

Note: without a paid Apple Developer account, this build is still
unsigned, so macOS's Gatekeeper will show an "unidentified developer"
prompt the first time you open it — right-click the app → Open, then
confirm. That's a one-time, normal step for unsigned apps and unrelated
to the malware-flagging issue the Electron build hit, since Tauri's
binary is far smaller and isn't a known target of that particular
false-positive signature.

**On the icon:** the project ships with a proper `.icns` (the multi-
resolution format macOS bundling actually requires) alongside the plain
PNG. If you ever swap in your own artwork, regenerate both with:
```bash
npx tauri icon path/to/your-source.png
```
