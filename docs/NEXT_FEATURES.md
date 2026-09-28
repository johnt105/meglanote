# MeglaNote — Features

## Ideas for next releases

Not yet agreed or specced. Just the candidates, roughly in priority order.

1. **Safety net for notes.** Some form of version history or periodic backup (e.g. a zipped copy of the notes folder on a schedule, or keeping previous versions of a note when it's overwritten). Plain `.md` files in iCloud are robust, but iCloud will sync a mistake or an accidental deletion everywhere.
2. **Clipboard capture.** A global hotkey that turns whatever's on the clipboard into a new note, or a passive clipboard-history panel.
3. **Feedback / bug report from inside the app**, so problems can be reported without leaving MeglaNote.

## Decided against (don't re-add without a new discussion)

- Roll-up of open action items per person.
- Automatic carry-over of action items between meetings, and any "smart" filtering built on either of the above.
- Auto-pinning meeting notes.
- Homebrew / Cask / personal tap distribution. The app is just the `.dmg` from GitHub Releases via `release.sh`, and the built-in Tauri updater handles every update after the first install.

## Built (from the 2026-09-06 spec, all shipped)

Kept as a record of how these were designed to behave.

- **Templates.** Plain `.md` files in a hidden `.templates` folder, only reachable through the template picker (never in the sidebar or search). Toolbar has `+ Blank note` (⌘N), `+ Meeting note` (⌘⇧N) and `New from Template` for anything else in the folder. The Meeting button loads whichever template is named "Meeting" at click time, and silently recreates the stock default if it's missing. Any note dropped into the folder becomes a template.
- **Meeting notes.** `{{date}}` / `{{time}}` are filled in once at creation. A real `meetingDate` frontmatter field is stamped for sorting, the note is auto-tagged `meeting`, and it's not auto-pinned.
- **Person notes.** A note is a person if it's in a `People` folder OR tagged `person`. `[[Name]]` never auto-creates a note; it's only created when the link is clicked. Once at least one meeting links to them, a "N meetings" pill under the title lists those meetings newest-first by `meetingDate`. This is computed on the fly (backlinks filtered and sorted) and never written to disk.
- **Backlinks.** Sorted newest-first (`meetingDate` if present, else the linking note's `updatedAt`), showing the last 5 with "+N more" to expand.
- **Icon.** Pen nib colour-blocked cream / dusty pink, dark spine line and breather hole, on a berry (#9c2f5e) tile.

---
Last updated 2026-09-28: 2026-09-06 spec marked as built; next-release ideas added.
