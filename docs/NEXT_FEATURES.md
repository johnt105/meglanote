# MeglaNote — Next Features Spec

Agreed scope from a design discussion. These are quality-of-life additions on top of the existing app — no changes to the file format, storage, or sync model. Every existing note keeps working exactly as it does today.

## 1. Templates

- Templates are plain `.md` files living in a reserved `Templates` folder.
- The `Templates` folder is hidden from normal sidebar/folder browsing and search — reachable only through the template picker (same treatment as the existing Trash folder gets).
- Toolbar gets two one-click buttons instead of the current single "+ New note":
  - `+ Blank note` — CmdOrCtrl+N (unchanged)
  - `+ Meeting note` — CmdOrCtrl+Shift+N (new)
  - Plus a `New from Template` dropdown for anything else sitting in the `Templates` folder — this is where future templates go (a recipe template, a journal template, anything added later) without needing a new dedicated button each time.
- The Meeting button is not hardcoded. At click time it loads whichever template file is named "Meeting" and creates a note from its current contents — so editing that file changes what the button produces.
- If the "Meeting"-named template is ever missing when the button is clicked, the app silently recreates a stock default rather than failing.
- Anyone can create their own template just by writing a note and dropping it in `Templates` — no separate template-editor UI needed.

## 2. Meeting template content

```markdown
# {{title}}
{{date}} · {{time}}

## Attendees
- [[ ]]
- [[ ]]

## Agenda
-
-

## Notes


## Decisions
>

## Action Items
- [ ]
- [ ]

```

- `{{date}}` / `{{time}}` are substituted once, at creation — plain text after that, no live-updating.
- The note also gets a real `meetingDate` field stamped into its frontmatter at creation (not just the `{{date}}` text) — this is the structured value everything below sorts by.
- Auto-tagged `meeting`.
- No auto-pin (explicitly decided against).

## 3. Person notes

- Detection rule (OR, not AND): a note counts as a person if it's in the `People` folder or carries the `person` tag. Covers both an organize-by-folder style and an organize-by-tag style without forcing either.
- Lazy note creation: typing `[[Name]]` never auto-creates a note — it stays a plain/unresolved reference. A note is only created the moment you click that link.
- History view: a computed, on-the-fly view (never written to disk) that appears on a person's note only once at least one meeting note links to them. Rendered as a pill/button just under the note's title, e.g. "Bob Smith / 12 meetings". Clicking it opens the list of linked meeting notes, sorted newest-first by `meetingDate`. This is backlinks, filtered to one person and sorted — not a new data structure.
- Explicitly cut from scope: a roll-up of open action items per person, and any automatic carry-over of action items between meetings.

## 4. Backlinks fix (applies to every note, not just people)

Today, backlinks are unsorted, uncapped, and can grow into an ever-expanding unscrollable strip.

Fix:
- Sort by date, newest first (`meetingDate` if present, else the linking note's `updatedAt`).
- Show the last 5, with a "+N more" to expand rather than an unbounded list.

## 5. Distribution

- No Homebrew, no Cask, no personal tap. Just ship the `.dmg` from GitHub Releases via `release.sh`. The Tauri auto-updater already configured in `tauri.conf.json` handles every update after the first install.

## Explicitly out of scope (parked or cut)

- Parked, not forgotten: a feedback/bug-report path; clipboard capture (global hotkey or passive clipboard-history panel).
- Cut for now: open-action-items roll-up per person; automatic action-item carry-over between meetings; any future "smart" filtering of the above.

## Icon

New app icon: a pen nib, color-blocked into two flat tones (cream / dusty pink) split down the spine, dark spine line and breather hole, on a solid berry (#9c2f5e) tile.

---
Last updated 2026-09-06, brought into a build session alongside the icon redesign.
