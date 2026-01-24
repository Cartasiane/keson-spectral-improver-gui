---
description: bump the version and trigger a new release
---

1. Ask the user for the new version number (e.g., 0.9.9) if it wasn't provided in the request.
2. Update the version field in `package.json`.
3. Update the version field in `src-tauri/Cargo.toml`.
4. Update the version field in `src-tauri/tauri.conf.json`.
5. Run `git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json`
6. Run `git commit -m "chore: bump version to <VERSION>"`
7. Run `git tag v<VERSION>`
8. Run `git push origin main`
9. Run `git push origin v<VERSION>`
10. Notify the user that the release workflow has been triggered and provide them with a link to the actions tab if possible, or just confirm success.
