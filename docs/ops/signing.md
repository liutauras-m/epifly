# Code-signing & notarization runbook

This document covers the one-time setup and ongoing rotation of signing credentials for
the ConusAI Browser Shell (macOS, Windows, iOS, Android) and the corresponding GitHub
Actions secrets.

---

## macOS (Developer ID + Notarization)

### Requirements
- Apple Developer Program membership
- Developer ID Application certificate in Keychain

### One-time setup

```bash
# Export the certificate as base64 for the CI secret
security find-certificate -c "Developer ID Application" -p | \
  openssl x509 -outform DER | base64 | pbcopy
# → paste into APPLE_CERTIFICATE secret

# The certificate password you set when exporting:
# → APPLE_CERTIFICATE_PASSWORD

# Signing identity (e.g. "Developer ID Application: Acme Inc (TEAMID)")
security find-identity -v -p codesigning | grep "Developer ID"
# → APPLE_SIGNING_IDENTITY

# Team ID (10-char alphanumeric from developer.apple.com)
# → APPLE_TEAM_ID
```

Store a notarytool keychain profile locally so the CI step can notarize:

```bash
xcrun notarytool store-credentials "ConusAI" \
  --apple-id "your@email.com" \
  --team-id  "YOURTEAMID" \
  --password "@keychain:AC_PASSWORD"   # app-specific password
```

### GitHub secrets to configure
| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64-encoded `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` export password |
| `APPLE_SIGNING_IDENTITY` | Full string from `security find-identity` |
| `APPLE_TEAM_ID` | 10-char team ID |

### Rotation
Certificates expire after 5 years. Re-export, re-base64, update the secret. Notarization
credentials (app-specific passwords) can be revoked/re-created in appleid.apple.com at
any time — update `APPLE_CERTIFICATE_PASSWORD` and re-run the `store-credentials` command
on the new runner.

---

## iOS (Ad-hoc / App Store distribution)

Signing for `tauri ios build` uses the same `APPLE_TEAM_ID`. For App Store uploads you
additionally need:

- Provisioning profile (`.mobileprovision`) stored as `APPLE_PROVISIONING_PROFILE` (base64)
- The bundle id must match `tauri.conf.json → identifier`: `com.conusai.browser`

---

## Windows (Authenticode)

The release workflow currently skips Authenticode signing. To enable it:

1. Obtain an EV Code Signing certificate from a trusted CA (DigiCert, Sectigo, etc.)
2. Export as `.pfx`
3. Add secrets:
   - `WINDOWS_CERTIFICATE` — base64 `.pfx`
   - `WINDOWS_CERTIFICATE_PASSWORD` — `.pfx` password
4. Add a signing step in `.github/workflows/release-shell.yml` before the MSI upload:
   ```yaml
   - name: Sign MSI
     run: |
       $cert = [Convert]::FromBase64String("${{ secrets.WINDOWS_CERTIFICATE }}")
       Set-Content -Path cert.pfx -Value $cert -Encoding Byte
       signtool sign /f cert.pfx /p "${{ secrets.WINDOWS_CERTIFICATE_PASSWORD }}" \
         /tr http://timestamp.digicert.com /td sha256 /fd sha256 \
         target\x86_64-pc-windows-msvc\release\bundle\msi\*.msi
   ```

---

## Android (Keystore)

### One-time setup

```bash
# Generate a keystore (keep the .jks file outside of git)
keytool -genkey -v \
  -keystore conusai-release.jks \
  -alias conusai \
  -keyalg RSA -keysize 4096 \
  -validity 10000
```

### GitHub secrets
| Secret | Value |
|---|---|
| `ANDROID_KEYSTORE_PATH` | path where CI writes the keystore (e.g. `/tmp/conusai.jks`) |
| `ANDROID_KEYSTORE_PASSWORD` | keystore password |
| `ANDROID_KEY_ALIAS` | `conusai` (or whatever alias you used) |
| `ANDROID_KEY_PASSWORD` | key password |

The CI job writes the keystore to `ANDROID_KEYSTORE_PATH` before calling `tauri android build`:

```yaml
- name: Write keystore
  run: echo "${{ secrets.ANDROID_KEYSTORE_B64 }}" | base64 -d > /tmp/conusai.jks
  env:
    ANDROID_KEYSTORE_PATH: /tmp/conusai.jks
```

### Rotation
Android keystores cannot be changed after an app is published to Google Play — losing the
keystore means losing the ability to update the published app. Store it in a password
manager (1Password, Bitwarden) and in an offline backup. Google Play App Signing
(where Google holds the upload key) is an alternative that removes this risk.

---

## Build-time sign-in configuration

Shell builds carry no credentials. Sign-in is Zitadel OIDC (authorization code + PKCE
through the system browser and a loopback redirect, `apps/browser-shell/src-tauri/src/auth/`);
the refresh token is kept in a Stronghold vault on the device. Three public values are
compiled in and must be set in the environment of every release build:

| Variable | Value |
|---|---|
| `VITE_GATEWAY_URL` | Public origin of the gateway service (its ConusAI Cloud public hostname) |
| `VITE_ZITADEL_ISSUER` | Zitadel issuer, `https://<AUTH_HOST>` |
| `VITE_ZITADEL_NATIVE_CLIENT_ID` | Client id of the Zitadel native application (see [deploy-conusai-cloud.md](../deploy-conusai-cloud.md)) |

A build without them starts but cannot sign in. `.github/workflows/release-shell.yml` does
not pass them yet; add them as repository variables before the next release.
