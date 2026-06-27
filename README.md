# wabba-tools

Tooling for archiving and serving the mod/modlist files behind Wabbajack
modlists. The workspace has three crates:

- **`wabba-tools`** — the CLI used to hash, validate, upload, sync, prune, and
  fetch mod/modlist files against a server.
- **`wabba-server`** — the backend that stores modlists and mods, tracks which
  files are archived, and serves them.
- **`wabba-protocol`** — shared data structures and the xxhash64 implementation
  used by both.

## CLI commands

Run `wabba-tools <command> --help` for full options.

| Command | What it does |
| --- | --- |
| `validate <WABBAJACK_FILE> <DOWNLOAD_DIR...>` | Check whether a modlist's required files are present locally. |
| `hash <FILE>` | Print the xxhash64 of a file. |
| `upload <SERVER> <FILE>` | Upload a single mod or modlist file to the server. |
| `sync <SERVER> <DIRECTORY>` | Hash every file in a downloads directory and upload anything the server doesn't already have. |
| `prune <SERVER> <DIRECTORY> --keep <XXHASH64>...` | Delete archived downloads not reachable from a `--keep` mod/modlist. Dry-run by default; pass `--dry-run false` to delete. |
| `fetch-mods <SERVER> <DIRECTORY> <MODLIST_XXHASH64>` | Download every mod a modlist requires that isn't already present locally. |

## FAQ

### My modlist installs fine from my downloads folder, but after I `sync` it the server still reports missing files. Why?

Because Wabbajack does **incremental installs**. When you upgrade a modlist in
place (or re-run an install into a directory that already has it), Wabbajack
reuses the files already on disk and only re-applies the steps that changed — so
it does **not** need every download archive present to finish. A clean install,
on the other hand, needs *every* required mod.

This means your downloads folder can be "complete enough" to upgrade/repair an
existing install while still missing files that a from-scratch install would
require — and those genuinely-missing files are exactly what the server reports.

To make sure your downloads folder is actually complete:

1. Point Wabbajack at a **brand-new, empty install directory** and start the
   install. This forces Wabbajack to download *every* required archive.
2. You can **cancel the install once the download phase finishes** — you only
   need the downloads, not the full install.
3. Re-run `sync`. The server should now have every file the modlist requires.
