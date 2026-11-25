Floe 0.4.1

- **Fix: installing for all users (`--system`) failed** with *"Flatpak system operation InstallBundle not allowed for user"*. Installing a bundle system-wide needs your administrator password, and Floe had told flatpak not to ask (`--noninteractive`). Floe now lets flatpak show the password prompt for system-wide installs; per-user installs are unchanged and need no password.
- If the password prompt is cancelled or can't be shown, the error explains why and gives the command to run in a terminal, and the downloaded bundle is kept for that.
- If Floe itself is installed for all users, a failed self-update also explains how to switch Floe to a per-user install, so later updates need no password.

**Updating from 0.4.0 when Floe is installed for all users:** 0.4.0 can't install this fix itself (that's the bug). Update once from a terminal:

```bash
gh release download v0.4.1 --repo tobeoffline64/Floe --pattern floe.flatpak --dir /tmp --clobber
flatpak install --system -y --reinstall /tmp/floe.flatpak
```
