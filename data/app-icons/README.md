# Icons of the apps Tessera sets up

Shown in the window when the app's own icon is not available (the app is not
installed). They come from the apps' free-software sources, under their
licenses:

| File | Source | License |
|---|---|---|
| `tessera-app-chromium.png` | Chromium, `chrome/app/theme/chromium/product_logo_256.png` | BSD-3-Clause (Chromium's `LICENSE`) |
| `tessera-app-okular.svg` | KDE Breeze icons, `icons/apps/48/okular.svg` | LGPL-3.0-or-later (Breeze's `COPYING-ICONS`) |
| `tessera-app-papers.svg` | GNOME Papers, `data/icons/scalable/apps/org.gnome.Papers.svg` | GPL-2.0 (Papers' `COPYING`) |

The logos of Firefox, Google Chrome, Microsoft Edge, Brave and PJeOffice are
not redistributable this way; those apps show their own icon when installed
(as a Flatpak or natively) and a generic one otherwise.

The names start with `tessera-app-` and not with Tessera's app ID, so Flatpak
does not export them to the desktop.
