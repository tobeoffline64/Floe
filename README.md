# Floe

Install and update flatpak apps straight from **GitHub releases**, without publishing them to
Flathub. Floe works like [Obtainium](https://github.com/ImranR98/Obtainium) on Android: keep a
list of repositories, check them for new releases, and install the attached `.flatpak` bundle
with one click.

Floe downloads with the **GitHub CLI** (`gh`) and installs with **flatpak**, so private
repositories work with your existing `gh auth login`, and Floe never stores a GitHub token.

Built in **Rust** with **[Dioxus](https://dioxuslabs.com) desktop** and the
**[Rust/UI](https://github.com/rust-ui/ui)** components and theme from
[PenguinFin](https://github.com/tobeoffline64/PenguinFin).

## Features

- **Repository list**: add `owner/name` or paste a GitHub link. Optional per repository: which
  release file to install (glob, default `*.flatpak`; an asset built for your CPU architecture
  is preferred when several match), the flatpak app ID, and whether to include pre-releases.
- **Checks**: one repository or all of them at once (in parallel, `Ctrl R`), automatically
  when Floe opens and every 1–24 hours while it's open, with a desktop notification for new
  releases.
- **Install / update**: `gh release download` → `flatpak install --reinstall`. Your app data
  in `~/.var/app/<id>` is kept. **Update all** installs every available update in turn.
- **Versions**: the installed version comes from `flatpak info` (so updates done outside Floe
  are noticed). The app ID is detected automatically after the first install if you don't
  enter it.
- **Live console** while downloading and installing: the `gh` and `flatpak` output as it
  arrives (progress bars redraw in place), a download progress bar, **Cancel** (stops the
  running command) and **Copy output**. When flatpak is quiet for a while, Floe explains
  why. The first install of an app also downloads its runtime, often several hundred MB.
- **Already installed?** When you add a repository, Floe looks through `flatpak list` and
  links an installed app with the ID `io.github.<owner>.<Name>`, or a bundle installed from a
  file with the same name, so it shows the installed version instead of reinstalling. An app
  from Flathub that only has a similar name is suggested (**Link**) but never linked
  automatically, because updating it from GitHub would replace the Flathub build.
- **Installed** page: every flatpak on this computer, which repository it belongs to,
  **Link** for suggested matches and **Track** for untracked GitHub-style IDs
  (`io.github.owner.Name` → `owner/Name`).
- Release notes, **Open app**, **Reinstall**, link to the release on GitHub.
- **Activity** page with the full `gh` / `flatpak` output of everything Floe did.
- **Settings**: `gh` and flatpak status, install for the user (`--user`, default) or system-wide
  (`--system`), keep or delete downloaded bundles, export/import the list (JSON in
  `~/Downloads`), dark mode, interface size (`Ctrl +` / `Ctrl −` / `Ctrl 0`).

## Requirements

```bash
sudo apt install gh flatpak        # Fedora: sudo dnf install gh flatpak
gh auth login
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```

Flathub is only needed for the runtimes the bundles depend on (for example GNOME 49).

## Track PenguinFin (or any other app)

1. **Add repository** → `tobeoffline64/PenguinFin` → **Add and check**.
2. Click **Install** (or **Update to v…**). Floe fills in the app ID
   (`io.github.tobeoffline64.PenguinFin`) after the install.

A repository works with Floe if its releases have a `.flatpak` bundle attached, which is what
PenguinFin's `.github/workflows/release.yml` (and Floe's own) publishes. If a release has
several bundles, set **Release file** to a pattern such as `*-x86_64.flatpak`.

## Install Floe

As a flatpak (from this repository's [latest release](https://github.com/tobeoffline64/Floe/releases/latest)):

```bash
gh release download --repo tobeoffline64/Floe --pattern floe.flatpak --dir /tmp --clobber
flatpak install --user -y --reinstall /tmp/floe.flatpak
flatpak run io.github.tobeoffline64.Floe
```

Then add `tobeoffline64/Floe` to its own list and it keeps itself up to date.

Inside the sandbox Floe runs `gh`, `flatpak`, `xdg-open` and `notify-send` **on the host**
with `flatpak-spawn --host`, because your GitHub login and flatpak installation live there.
The manifest grants `--talk-name=org.freedesktop.Flatpak` for this, which is effectively full
access to your session (as any app installer needs). If you'd rather not grant that, run the
native build instead:

```bash
cargo install --path .     # installs ~/.cargo/bin/floe
floe
```

## Build from source

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libsoup-3.0-dev   # Ubuntu deps
npm install                                       # Tailwind CLI (only to change styles)
npx @tailwindcss/cli -i style/app.css -o assets/app.css --minify
cargo run --release
cargo test
```

Build the flatpak bundle like PenguinFin (needs `flatpak` and `flatpak-builder`):

```bash
./build-flatpak.sh            # fast: incremental, no LTO
./build-flatpak.sh --release  # incremental with the release profile (thin LTO)
./build-flatpak.sh --clean    # from scratch in flatpak-builder (what the GitHub release uses)
```

### Releasing

Same as PenguinFin: bump `version` in `Cargo.toml` (and `flatpak/*.metainfo.xml`), update
`.github/release-notes.md`, push to `main`. The workflow builds the flatpak and publishes the
`v<version>` release with `floe.flatpak` attached.

### Environment variables

| Variable | Effect |
| --- | --- |
| `FLOE_CONFIG` | Use another list/settings file (default `~/.config/floe/config.json`) |
| `FLOE_GH`, `FLOE_FLATPAK` | Use another `gh` / `flatpak` program (the tests use fakes in `tests/fake/`) |
| `FLOE_WINDOW_SIZE=900x700` | Initial window size |
| `FLOE_COMPAT_RENDERING=1` | Slower, most compatible rendering (blank or flickering window on some drivers) |
| `FLOE_SLOW=1` | Debug builds: delay background work to inspect loading states |

## Layout

| Path | What |
| --- | --- |
| `src/config.rs` | Tracked repositories, settings, update status logic, JSON storage, export/import |
| `src/gh.rs` | `gh` wrapper: latest release, asset matching, download, login status |
| `src/flatpak.rs` | `flatpak` wrapper: install bundle, installed apps and matching, app ID detection, launch |
| `src/host.rs` | Runs host programs (via `flatpak-spawn --host` when sandboxed), streaming output |
| `src/updater.rs` | Check / download / install steps, linking installed apps |
| `src/console.rs` | Live output and cancellation of running jobs |
| `src/version.rs` | Version and tag comparison |
| `src/app.rs` | App shell, sidebar, state and background actions |
| `src/views/` | Apps, Installed, Activity and Settings pages |
| `src/ui/` | Rust/UI components (MIT; see `LICENSE-rust-ui`), shared with PenguinFin |
| `style/app.css` | Tailwind source with the PenguinFin theme tokens; compiled to `assets/app.css` |
| `flatpak/` | Manifest, desktop file, AppStream metadata, icon |
