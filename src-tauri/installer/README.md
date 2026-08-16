# Windows installer branding

The NSIS and MSI bundles use the SpringCat artwork in this directory. The
generated BMP files have the exact dimensions required by the two Windows
installer toolchains and are referenced from `../tauri.conf.json`.

To regenerate the installer artwork after changing `../icons/icon.png` or the
background source images, run this command from the repository root on Windows:

```powershell
pnpm installer:assets
```

Do not edit the BMP files by hand. Update the source PNG files under `source/`
and regenerate them instead.
