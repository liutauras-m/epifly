# ConusAI Browser Shell — User Guide

The ConusAI Browser Shell is a native Tauri 2 application that brings the ConusAI agent interface to macOS, Windows, Linux, iOS, and Android. This guide covers installation, signing in, recording sessions, replay, and troubleshooting.

> Sign-in currently works in the desktop builds (macOS, Windows, Linux) only. Sign-in on iOS and Android is not yet implemented, so the mobile builds stop at the sign-in screen.

---

## Quick Start: Download and Install

### macOS

1. Download the `.dmg` installer from the [Releases page](https://github.com/conusai/browser-shell/releases).
2. Open the `.dmg`, drag **ConusAI** into your Applications folder.
3. Launch **ConusAI** from Applications or Spotlight.
4. If macOS Gatekeeper blocks the app on first launch, go to **System Settings → Privacy & Security** and click **Open Anyway**.

### Windows

1. Download the `.msi` installer from the [Releases page](https://github.com/conusai/browser-shell/releases).
2. Run the installer and follow the prompts.
3. If Windows SmartScreen shows a warning, click **More info → Run anyway**. The binary is code-signed; the warning appears because the certificate is not yet widely trusted.

### Linux

1. Download the `.AppImage` from the [Releases page](https://github.com/conusai/browser-shell/releases).
2. Make it executable: `chmod +x ConusAI-*.AppImage`
3. Run it directly: `./ConusAI-*.AppImage`
4. Optionally, integrate with your desktop environment using [AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher).

### iOS (TestFlight)

1. Install [TestFlight](https://apps.apple.com/app/testflight/id899247664) from the App Store.
2. Tap the TestFlight invitation link provided by your administrator.
3. Accept the invitation and install the **ConusAI** beta.

### Android (APK)

1. Enable **Unknown sources** (or **Install unknown apps**) for your browser or file manager in **Settings → Security**.
2. Download the `.apk` from the link provided by your administrator.
3. Open the downloaded file and tap **Install**.
4. After installation, you can re-disable unknown sources if preferred.

---

## First Launch: Signing In

Each build of the shell is made for one ConusAI instance: the gateway URL and the
sign-in service (Zitadel) are fixed when the app is built. There is nothing to
paste or configure.

1. Launch the app. It shows the sign-in screen.
2. Click **Sign in**. Your default system browser opens your organisation's sign-in page.
3. Sign in with your organisation account (the same one you use for the web app).
4. The browser shows a short confirmation page; return to the app. You are signed in.

The browser hands the result back to the app over a one-time local address on
your computer (`http://127.0.0.1:<port>/callback`), so a firewall prompt about a
local connection may appear on first sign-in. Sign-in times out after five
minutes.

Your organisation and plan come from your account; the shell never asks for
them. The app keeps a long-lived refresh token in an encrypted vault in its
app-data directory and fetches short-lived access tokens with it, so you stay
signed in across restarts.

**Signing out** (profile menu → **Sign out**) revokes the refresh token, clears the
vault and opens the sign-in service's logout page in your browser, so the next
sign-in asks for your credentials again.

---

## Recording a Session

Sessions let the agent observe your browser activity to answer questions or automate tasks.

### Starting a recording

1. Open the **Agent** panel (sidebar or hamburger menu on mobile).
2. Tap **Start Recording** (circle icon). The shell begins capturing browser events.
3. A recording indicator (red dot) appears in the toolbar.

### What gets captured

- Page URLs and titles
- DOM structure snapshots at configurable intervals
- User interaction events (clicks, form field changes, scroll positions)
- Network request metadata (method, URL, status code) — **not** request or response bodies

### What is never captured

- Passwords and fields marked `type="password"`
- Credit card numbers and other payment data (detected via Luhn check and common field-name heuristics)
- Content inside `<iframe>` elements from cross-origin domains
- Anything explicitly excluded via the admin-configured allow-list

### Stopping a recording

Tap **Stop Recording** (square icon) in the toolbar. The session is finalised and sent to the agent for processing. You can also stop by closing the observed tab.

---

## Replay (Dry-Run Mode)

Replay lets the agent re-execute a recorded session in a sandboxed context to verify that an automation script works correctly before applying it to live data.

### What dry-run means

In dry-run mode the agent steps through the recorded interaction sequence, but all network requests that would mutate server state (POST, PUT, PATCH, DELETE) are intercepted and logged without being sent. Read-only requests (GET) are executed normally so the UI renders authentically.

### Triggering replay from agent chat

1. In the **Agent** panel, type a message such as `Replay the last session` or `Dry-run the invoice submission workflow`.
2. The agent identifies the matching recorded session and asks for confirmation.
3. Confirm by typing `yes` or clicking **Run dry-run**.
4. The Browser Shell opens a replay view showing each step with its intercepted or executed status.
5. The agent reports a summary of what would have changed if the run had been live.

---

## Troubleshooting

### macOS — app blocked by Gatekeeper

**Symptom:** "ConusAI cannot be opened because the developer cannot be verified."

**Fix:** Go to **System Settings → Privacy & Security**, scroll to the bottom, and click **Open Anyway** next to the ConusAI entry. Alternatively, right-click the app in Finder and select **Open**.

### macOS — shell not found after DMG install

**Symptom:** ConusAI does not appear in Applications.

**Fix:** Confirm you dragged the application icon (not the DMG background) into the Applications folder. Eject and re-open the DMG to retry.

### Windows — SmartScreen warning on launch

**Symptom:** "Windows protected your PC."

**Fix:** Click **More info**, then **Run anyway**. The binary is signed; the warning is because the certificate reputation is still accumulating. The warning will disappear after enough installs.

### Windows — installation fails with error 1603

**Symptom:** MSI installer exits with code 1603.

**Fix:** Run the installer as Administrator (right-click → Run as administrator). Ensure no previous version is partially installed; use **Add or Remove Programs** to uninstall any prior version first.

### Android — "App not installed" error

**Symptom:** Installation fails after tapping the APK.

**Fix:** Check that **Install unknown apps** is enabled for the app you used to open the APK (e.g. Files, Chrome). Also verify that there is sufficient storage space. If the error persists, download the APK again — partial downloads fail silently.

### Android — app crashes on launch

**Symptom:** App closes immediately after the splash screen.

**Fix:** Ensure your device runs Android 10 (API 29) or later. If the device meets the requirement, go to **Settings → Apps → ConusAI → Permissions** and grant the required permissions (network access, storage).

### All platforms — sign-in does not finish

**Symptom:** The browser shows the sign-in page, but the app stays on the sign-in screen or reports an error.

**Fix:**
- Complete the sign-in within five minutes, then click **Sign in** again.
- Allow the local connection if a firewall asks: the browser returns to the app via `127.0.0.1`.
- "no refresh token" or a redirect error from the sign-in page means the instance's Zitadel native application is misconfigured; tell your administrator (see [deploy-conusai-cloud.md](deploy-conusai-cloud.md)).

### All platforms — "Your session expired. Please sign in again."

**Symptom:** The app returns to the sign-in screen with this message.

**Fix:** Sign in again. It happens when your administrator ended your session or the refresh token expired.

### All platforms — recordings not uploading

**Symptom:** Sessions stay in "pending upload" state after stopping.

**Fix:** Check your network connection. The shell retries uploads automatically with exponential back-off. If the problem persists, go to **Settings → Diagnostics → Flush Upload Queue** to force an immediate retry.

---

## Privacy: Data Storage and Deletion

### What data is stored and where

| Data | Storage location | Retention |
|---|---|---|
| Recorded session events | Encrypted local cache, then uploaded to your organisation's RustFS bucket | Configurable by admin (default: 90 days) |
| Sign-in refresh token | Encrypted Stronghold vault in the app-data directory (key derived with Argon2 over a per-install salt) | Until you sign out or it expires |
| Agent chat history | Your organisation's Postgres instance | Configurable by admin |
| Diagnostic logs | Local app data directory | 7 days rolling |

The Browser Shell does not send data to Anthropic or any third party. All traffic goes directly to your organisation's ConusAI gateway.

### How to delete your data

**Delete a single session:** In the **History** panel, long-press or right-click the session and select **Delete**.

**Delete all local data:** Go to **Settings → Privacy → Clear All Local Data**. This removes the local cache and diagnostic logs but does not delete data already uploaded to your organisation's servers.

**Request server-side deletion:** Contact your ConusAI administrator. Admins can delete sessions and chat history from the Foundry admin console under **Admin → Data Management**.

**Sign out of the device:** Choose **Sign out** in the profile menu. This revokes the refresh token at the sign-in service and deletes it from the vault. An administrator can also end your sessions in the sign-in service (Zitadel).
