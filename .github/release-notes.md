Floe 0.3.0

- **Floe updates itself.** Settings → *Floe updates* shows your version, checks `tobeoffline64/Floe` for a newer release (automatically when Floe opens and with the scheduled checks, or with *Check for updates*), installs it with the same live console and Cancel as app installs, then offers **Restart Floe**. The Settings item gets a badge and you get a desktop notification when a new version is out. Floe installs into the same installation it runs from (user or system).
- Running a build from source? The card says so: updating there installs the flatpak alongside it, and shows how to update the source build instead.

From 0.2.0 and earlier, update once the usual way (Floe's own list, or `gh release download --repo tobeoffline64/Floe --pattern floe.flatpak --dir /tmp --clobber && flatpak install --user -y --reinstall /tmp/floe.flatpak`); from then on use Settings.
