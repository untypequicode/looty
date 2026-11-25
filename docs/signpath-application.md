# Windows code signing with SignPath Foundation (prepared, not applied)

Issue #85. Status: **nothing has been applied for.** The CI steps exist but are switched off.

## What is and is not signed today

- **Updates (done, free):** the Tauri updater key signs the installer (`*-setup.exe.sig`); the app checks the fixed `updater` release `latest.json` every 6 hours and verifies the signature. See `RELEASING.md`.
- **Authenticode (not done):** Windows SmartScreen still says "unknown publisher" on a first install. A self-signed certificate does **not** help: Windows does not trust it, it shows the same warning. A paid certificate (OV or EV) also builds SmartScreen reputation over downloads; EV used to bypass it instantly, which is no longer guaranteed. The only free route is SignPath Foundation, and even a valid signature may need some downloads before the warning fully disappears.

## SignPath Foundation requirements (from signpath.org/terms and about.signpath.io, checked 2026-11-25)

- OSI-approved license, no commercial dual-licensing, no proprietary component. Looty is GPL-3.0: fine.
- Actively maintained, **already released** in the form to be signed, functionality described on the download page.
- Not malware or hacking tools (no features that look for or exploit vulnerabilities). Respect privacy, announce system changes, provide uninstallation (the NSIS installer has an uninstaller).
- Built from source in a verifiable way on **GitHub-hosted runners** (the Desktop workflow uses `windows-2025`).
- Team roles: Authors, Reviewers (every non-committer change reviewed), Approvers (**every release needs manual approval** in SignPath). MFA required on GitHub and SignPath for all members.
- The project home and download pages must show a **Code signing policy** section (text below).
- The certificate is issued to SignPath Foundation (they are the publisher shown to users). Product name and version metadata of signed files must be set and constant (the Tauri `productName` is `Looty`).
- Acceptance is at their discretion and for executables they require "a certain verifiable reputation". A young single-maintainer project may be refused or asked to wait: say so honestly in the application.

## Application (apply at https://signpath.org/apply)

> **Project:** Looty, a self-hosted collaborative workspace for students and teams (notes, tasks, calendar, files, Moodle integration) with a Windows/Linux desktop app and an Android app.
> **Repository:** https://github.com/untypequicode/looty (public)
> **License:** GNU GPL v3 (OSI-approved, no dual licensing, no proprietary component).
> **What would be signed:** the Windows NSIS installer of the desktop app (Tauri v2, `Looty_x.y.z_x64-setup.exe`), published on GitHub Releases (tag `desktop-v*`), on https://looty.untypequicode.fr and on winget (`untypequicode.Looty`).
> **Release process:** releases are produced only by the GitHub Actions workflow `Release` from `backend/releases.json` on `main`; the installer is built by `.github/workflows/desktop.yml` on a GitHub-hosted `windows-2025` runner, and the build is submitted with `signpath/github-action-submit-signing-request`. No binary is built or signed elsewhere.
> **Team:** one maintainer (Authors, Reviewers and Approvers: @untypequicode), MFA enabled on GitHub and SignPath. Changes go through pull requests checked by CI.
> **Privacy:** the desktop app only talks to the Looty instance the user configures, plus the update check on github.com (Tauri updater, signature-verified). Nothing is sent elsewhere.
> **Already released:** desktop 0.8.x (first Windows release; see https://github.com/untypequicode/looty/releases).
> **Reputation:** [fill in: stars, users, downloads, mentions]. We know the project is young and accept to wait or reapply.

## What the repository must show before applying

1. Public repo, `LICENSE` (present), README describing the app (present).
2. A "Code signing policy" section on the README and the download page. Draft (add only once approved, it would be false before):

   ```md
   ## Code signing policy

   Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

   - Committers and reviewers: [untypequicode](https://github.com/untypequicode)
   - Approvers: [untypequicode](https://github.com/untypequicode)

   Privacy policy: the desktop app does not transfer any information to other networked systems unless specifically requested by the user (it contacts the Looty instance you configure and checks github.com for updates).
   ```
3. Release notes and a download page describing the app (present on the site).

## Steps for the maintainer

1. Turn on MFA on GitHub (and later SignPath). Open https://signpath.org/apply and paste the text above.
2. When approved, in SignPath: create project `looty` with an artifact configuration for a single PE file, e.g. `<artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1"><pe-file><authenticode-sign/></pe-file></artifact-configuration>` (add product name `Looty` metadata restrictions as they require), add the trusted build system "GitHub.com" and link it, create a `release-signing` policy with manual approval, and install the SignPath GitHub App on the repo.
3. In the repo: `gh secret set SIGNPATH_API_TOKEN` (submitter token), then
   `gh variable set SIGNPATH_ORGANIZATION_ID --body <id>`, `gh variable set SIGNPATH_PROJECT_SLUG --body looty`, `gh variable set SIGNPATH_SIGNING_POLICY_SLUG --body release-signing`, and finally `gh variable set SIGNPATH_ENABLED --body true`.
4. Add the Code signing policy section (above) to the README and the site, update the "unknown publisher" sentences (README, `desktop/README.md`, site i18n and docs, `desktop/tests/windows/CHECKLIST.md`, `RELEASING.md`).
5. At the next desktop release, approve the request in SignPath (the workflow waits up to 24 hours).

## How the CI behaves when enabled

In `.github/workflows/desktop.yml` (Windows job, not on pull requests, only when `vars.SIGNPATH_ENABLED == 'true'`): the NSIS installer is uploaded unsigned, signed by SignPath, copied over the original, and the updater `.sig` is **recomputed** on the signed file (the updater signature covers the exact bytes). Untested until SignPath approves; check the first run by hand, including `Get-AuthenticodeSignature` on the installer and one in-app update.
