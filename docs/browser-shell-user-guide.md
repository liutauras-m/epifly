# ConusAI Browser Shell — User Guide

The ConusAI Browser Shell is a native Tauri 2 application that brings the ConusAI agent interface to macOS, Windows, Linux, iOS, and Android. This guide covers installation, first-launch provisioning, recording sessions, replay, and troubleshooting.

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

## First Launch: Device Token Provisioning

### What is a device token?

A device token is a short-lived credential that binds your device to your organisation's ConusAI workspace. It authorises the shell to connect to the agent gateway and identifies your device in audit logs. Tokens are scoped to a tenant and expire after a configurable period (default: 30 days).

### How an admin issues a device token

Your ConusAI administrator issues device tokens from the admin console:

1. Log in to the Foundry UI (`https://<your-instance>/`) as a super-admin.
2. Navigate to **Admin → Device Tokens → New Token**.
3. Enter a display name for the device (e.g. "Alice's MacBook") and select the appropriate tenant and plan.
4. Click **Generate**. Copy the token — it is shown only once.
5. Share the token securely with the device owner (e.g. via a password manager or an encrypted message).

### Entering your token

On first launch, the Browser Shell shows a **Provision Device** screen:

1. Paste the token into the **Device Token** field.
2. Optionally enter the **Gateway URL** if your instance is self-hosted (e.g. `https://api.example.com`).
3. Tap or click **Connect**.

The shell verifies the token against the gateway. On success, it saves the token in the system keychain and will refresh it automatically before expiry. You will not need to re-enter the token unless it is revoked.

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

### All platforms — token provisioning fails

**Symptom:** "Invalid or expired device token" error on first launch.

**Fix:** Ask your administrator to issue a new token. Tokens expire after 30 days by default and can only be used once. Ensure the **Gateway URL** field matches your organisation's instance URL exactly (no trailing slash).

### All platforms — recordings not uploading

**Symptom:** Sessions stay in "pending upload" state after stopping.

**Fix:** Check your network connection. The shell retries uploads automatically with exponential back-off. If the problem persists, go to **Settings → Diagnostics → Flush Upload Queue** to force an immediate retry.

---

## Privacy: Data Storage and Deletion

### What data is stored and where

| Data | Storage location | Retention |
|---|---|---|
| Recorded session events | Encrypted local cache, then uploaded to your organisation's RustFS bucket | Configurable by admin (default: 90 days) |
| Device token | System keychain (macOS Keychain, Windows Credential Manager, Android Keystore) | Until revoked or expired |
| Agent chat history | Your organisation's Postgres instance | Configurable by admin |
| Diagnostic logs | Local app data directory | 7 days rolling |

The Browser Shell does not send data to Anthropic or any third party. All traffic goes directly to your organisation's ConusAI gateway.

### How to delete your data

**Delete a single session:** In the **History** panel, long-press or right-click the session and select **Delete**.

**Delete all local data:** Go to **Settings → Privacy → Clear All Local Data**. This removes the local cache and diagnostic logs but does not delete data already uploaded to your organisation's servers.

**Request server-side deletion:** Contact your ConusAI administrator. Admins can delete sessions and chat history from the Foundry admin console under **Admin → Data Management**.

**Revoke your device token:** Ask your administrator to revoke your token under **Admin → Device Tokens**. Revocation takes effect immediately and prevents further uploads from that device.
