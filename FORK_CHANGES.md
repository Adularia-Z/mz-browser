# Adularia maintenance fork

This fork is maintained at https://github.com/Adularia-Z/mz-browser and retains
the upstream copyright notices and AGPL-3.0 license. It is based on upstream
commit `8de3c7fc2581a96b51643f984a7090aa699cd578`, including the environment
transfer fixes made after upstream v0.0.1.

## Changes in 0.0.2

- Uninstall checks whether the application is running before any hook or deletion.
- Only package-owned files are removed. Unrelated files and nonempty folders in
  the installation directory are preserved.
- Browser profiles, accounts, settings, custom cache/log folders and runtime path
  settings are retained on uninstall. The unsafe data-deletion checkbox is removed.
- Sensitive-field encryption errors are propagated to the caller; secrets are
  never saved as plaintext when Windows DPAPI fails.
- Application updates use this fork's release endpoint and signing key, preventing
  an upstream release from replacing these changes.
- The Windows build runs frontend, Rust, MCP and installer policy tests, then
  installs and uninstalls the actual package with unrelated-file sentinels.

Browser kernels still come from the upstream browser-kernel releases and are
verified against the application's kernel catalog. They are not rebuilt by this
fork. Kernel-level fingerprint behavior is outside these maintenance changes.

## Installation

Run the x64 installer from this fork's Releases. Use a dedicated program directory.
Uninstall removes the application but retains all browser data. Remove data
manually only after confirming the location and making any required backup.

The Tauri updater signature authenticates releases of this fork. It is separate
from Windows Authenticode; these installers do not have a Windows publisher certificate.
