# Runbook: F-Droid (issues #167, #168)

Two separate things: Looty's own repository (automatic, #167) and the main F-Droid catalogue (a metadata merge request, #168). Design: [ADR 0016](adr/0016-android-fdroid-variant-and-repository.md).

## Looty's own repository (#167)

Published at `https://looty.untypequicode.fr/fdroid/repo` by the **F-Droid** workflow, which **Release** runs after each Android release (and which can be started by hand from Actions, "F-Droid", Run workflow: it builds the unsigned `fdroid` APK from the chosen ref and publishes it).

- Repository key: a dedicated PKCS12 keystore (RSA 4096, alias `looty-fdroid-repo`), distinct from the app signing key. The keystore and its passwords are kept by the maintainer outside the repository (`~/.looty-secrets/`, mode 700) and in the secrets `FDROID_KEYSTORE_B64`, `FDROID_KEYSTORE_PASSWORD`, `FDROID_KEY_ALIAS`, `FDROID_KEY_PASSWORD`. Back the keystore up: if it is lost, every client has to re-add the repository with a new fingerprint.
- Upload: `SITE_UPLOAD_TOKEN` (GitHub secret) must equal `UPLOAD_TOKEN` of the site container (`SITE_UPLOAD_TOKEN` in the server's `.env`, read by `docker-compose.site.yml`). The site server accepts the `fdroid/` folder; the deployed site image must be recent enough to contain it (otherwise the upload step fails with HTTP 400: redeploy the site).
- Key fingerprint (SHA-256, public): `98AB977C06DF41102B93943862203BDFABF197A589B68955A3138882FB6D218C`. It is shown on the documentation page "Android: the Looty F-Droid repository" and served at `/fdroid/fingerprint.txt`; the QR code at `/fdroid/repo/index.png` contains it.
- Check: add `fdroidrepos://looty.untypequicode.fr/fdroid/repo` in F-Droid or Droid-ify; Looty must appear, and F-Droid must show the fingerprint above.

Rotating the repository key means new secrets and a new fingerprint on the documentation page; users must re-add the repository.

## Main F-Droid catalogue (#168)

Ready in the repository: `packaging/fdroid/io.github.untypequicode.looty.yml` (the `fdroiddata` metadata: build recipe for the `fdroid` flavour, `UpdateCheckMode: Tags ^android-v`, `AutoUpdateMode: Version`, categories, `GPL-3.0-only`) and `fastlane/metadata/android/` (title, descriptions, changelogs, checked by CI). No anti-feature is declared: the app only talks to the server the user chooses, which is free software (no `NonFreeNet`), has no tracker and no proprietary dependency; ask the reviewers to confirm. The metadata pins version 0.32.0 (code 37): update `versionName`, `versionCode`, `commit` and `CurrentVersion*` to the latest `android-v*` tag before submitting.

Steps for the maintainer (a GitLab account is needed):

1. Create an account on https://gitlab.com and fork https://gitlab.com/fdroid/fdroiddata (button "Fork").
2. Clone your fork and add the file:
   ```sh
   git clone https://gitlab.com/<you>/fdroiddata.git && cd fdroiddata
   git checkout -b io.github.untypequicode.looty
   cp /path/to/looty/packaging/fdroid/io.github.untypequicode.looty.yml metadata/
   git add metadata && git commit -m "New app: Looty (io.github.untypequicode.looty)" && git push -u origin HEAD
   ```
3. Open a merge request to `fdroid/fdroiddata` (GitLab shows the link after the push). Use the project's merge request template and tick its checklist; the CI of the MR runs `fdroid lint`, `fdroid build` and the scanner, fix what it reports (the file can be edited in the branch).
4. Answer the reviewers (days to weeks). Remarks to expect: the build tools (`compileSdk` 37, build-tools 37.0.0, Gradle 9.8.1 and JDK 17 must exist on the F-Droid build server; the reviewers tell you), screenshots (none yet: add PNGs under `fastlane/metadata/android/<locale>/images/phoneScreenshots/` in the repository and release), and the signing note: the catalogue signs with F-Droid's key, so moving between the catalogue and the direct APK or Looty's repository needs a reinstall (already stated in the documentation).

The F-Droid build cannot be run in this environment; `fdroid build` of the metadata is only verified by the merge request CI. The Android workflow already builds the `fdroid` flavour twice and fails if the two APKs differ.
