Floe 0.4.0

**Beta updates.** Every app card now has an *Updates: Stable | Beta* switch.

- **Stable** (the default) installs regular releases only. When a newer beta exists (a GitHub release marked as a pre-release, like PenguinFin's `v2.0.0-beta.1`), the card tells you, with the beta's notes and a **Switch to beta** button.
- **Beta** installs the newest of the latest beta and the latest stable release, so once a final version ships you move onto it automatically.
- Switching back to **Stable** while a beta is installed shows *On beta*, with **Stay on beta** or **Return to stable** (a confirmed downgrade; check the beta's notes about your data first).
- Betas that name their bundle differently work out of the box with `*.flatpak`; otherwise set *Beta release file* under Edit.
- **Settings → Floe updates → Floe beta updates** does the same for Floe itself.

Update from **Settings → Floe updates**.
