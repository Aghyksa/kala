# Kala

A local time tracking workspace for macOS, with fast menu bar capture. Track work, explore your dashboard, and build reports with project-specific fields. Ask your AI how many hours you worked through MCP.

Everything stays on your Mac: one SQLite file, no account, no server, no telemetry.

<p align="center"><img src="assets/screenshots/popover.png" width="380" alt="Kala menu bar popover"></p>

## Install

1. Download `Kala_x.y.z_universal.dmg` from [Releases](../../releases).
2. Drag Kala to Applications.
3. Builds are not signed yet. The first time, right-click Kala.app and choose **Open**, or run:

```bash
xattr -dr com.apple.quarantine /Applications/Kala.app
```

Kala opens its desktop workspace on launch. Open **Settings** to configure projects and record a global shortcut. Launch at login runs quietly in the menu bar.

## Workspace

Click **Open Kala** in the menu bar popover to open the desktop app. Closing the window leaves Kala running in the menu bar.

- **Dashboard** shows total time, project breakdowns, daily activity, and recent entries for the selected period.
- **Timesheet** lets you search, filter, edit entries, and choose columns. Entries are paginated; totals cover every matching entry.
- **Projects** lets you create, archive, and restore projects, configure aliases and rounding, and manage custom fields. Each project can have text, number, date, select, checkbox, and URL fields, with separate defaults for table and export visibility. Archiving a field preserves existing values.
- **Reports** adds grouping by project, day, or custom field, exact field-value filters, saved views, and selected-column CSV/Excel exports. Exports include every matching entry and hours, without prices.

Project settings are also available in **Settings → Projects**. Manual entries use explicit start and end dates; existing multi-day timer entries keep their original timestamps when their description or custom fields change.

## Quick entry

Press your shortcut (or click the menu bar icon) and type:

```
[date] <project> [time] <description>
```

| You type | You get |
|---|---|
| `acme 2h fix login` | Acme, today, 2 hours ending now |
| `globex 13-15 sprint meeting` | Globex, today 13:00–15:00 |
| `yesterday acme 1.5h review` | Acme, yesterday, 1.5 hours after your last entry that day |
| `mon globex 30m standup` | Globex, last Monday |
| `16/10 acme 9-11.30 deploy` | Acme, 16 October, 09:00–11:30 |
| `acme refactor auth` | starts a timer for Acme |

- Durations: `2h`, `2j`, `1.5h`, `1,5j`, `30m`, `1h30m`.
- Ranges: `13-15`, `13.30-15`, `13:30-15:00`.
- Dates: `yesterday`/`kemarin`, weekday names in English or Indonesian, `16/10`, `2026-10-16`.
- Project names match by prefix or alias (`ac` → Acme).

Times Kala fills in for you (durations, stopped timers) follow the project's rounding: start rounds down, length rounds to the nearest unit. Ranges you type are kept as typed.

## Timer

Click a project in the popover, or press `⌘1`–`⌘9` while it is open. Starting another project stops the running one. The running timer shows in the menu bar.

## Export

Popover → **Export** → pick project and month. Kala writes `Kala-<Project>-YYYY-MM.xlsx` (or `.csv`) to Downloads: title, period, one row per entry, hours as decimals, and total hours.

## MCP

Kala ships an MCP server in the same binary. Settings → MCP shows the exact command for your install. For Claude Code:

```bash
claude mcp add --scope user kala -- /Applications/Kala.app/Contents/MacOS/kala mcp
```

Tools: `log_time`, `list_entries`, `update_entry`, `delete_entry`, `summary`, `export`, `timer_start`, `timer_stop`, `timer_status`.

Then ask things like "log 2 hours on Acme for the login fix" or "how many Acme hours this month?".

## Build from source

Requires Rust (stable) and Node 22+.

```bash
npm install
npm run tauri dev
```

While `npm run tauri dev` is running, frontend edits reload and Rust edits rebuild the app. Quit an installed copy before starting development; Kala uses a single application instance.

To build a standalone app, stop development and run:

```bash
npm run tauri build -- --bundles app
```

The app is written to `target.noindex/release/bundle/macos/Kala.app`. Quit Kala, then replace the copy in Applications to update the installed app. Source edits do not update an installed copy automatically.

```bash
cargo test --workspace
npm run check
```

The npm Tauri script also searches standard rustup installation paths. Direct Cargo commands require Cargo in your shell PATH.

`KALA_DB=/path/to/test.db` points Kala at another database, handy while developing.

Project layout: `core/` is plain Rust (parser, rounding, SQLite store, export) with unit tests. `src-tauri/` is the menu bar shell and the `kala mcp` server. `src/` is a small Svelte UI.

## License

MIT
