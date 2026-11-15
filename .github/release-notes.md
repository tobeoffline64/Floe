Floe 0.2.0

- **Live console** for every download and install: see `gh` and `flatpak` output as it happens, with a download progress bar, **Cancel**, and **Copy output**. If flatpak goes quiet (for example while it downloads an app's runtime the first time), Floe says so instead of looking frozen.
- **Already-installed apps are found and linked** when you add a repository: an installed app with the ID `io.github.<owner>.<Name>`, or a bundle with the same name, shows its installed version instead of offering a reinstall. Apps from Flathub with a similar name are only suggested.
- New **Installed** page: every flatpak on this computer, which repository it belongs to, and one-click **Track** for GitHub-hosted apps.

Update from Floe itself, or: `gh release download --repo tobeoffline64/Floe --pattern floe.flatpak --dir /tmp --clobber && flatpak install --user -y --reinstall /tmp/floe.flatpak`
