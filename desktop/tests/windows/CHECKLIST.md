# Windows: checks on a real PC

The **Windows installer test** job (`.github/workflows/desktop.yml`, script `smoke.ps1`) checks the following on a clean Windows runner:

- silent install and uninstall;
- `looty://` links and the notification identity in the registry;
- start-up and the main window;
- the page served through the local proxy, which refuses other programs;
- the session kept (and encrypted) after a restart;
- a `looty://` link reaching the open window;
- hidden start;
- offline and back online;
- cleanup on uninstall.

A runner can't click a notification, open the taskbar menu or press global shortcuts. Check these once on a Windows 10 or 11 PC, with the installer from the release (`Looty_<version>_x64-setup.exe`).

Allow about 10 minutes. Note anything that differs from what's written here.

## Install

1. Run the installer. Windows SmartScreen warns about an unknown publisher (the app is not signed yet): **More info → Run anyway**.
   The installer must not ask for administrator rights.
2. Looty appears in the Start menu. Launch it, enter your server address and sign in.

## Notifications

3. From another account (or your phone), assign yourself a task in a space you share.
   A Windows notification "Looty" appears within a minute.
4. Click the notification body: Looty comes to the front on that task.
5. Assign yourself a task due today, wait for the "due soon" notification and click **Mark as done**.
   The task is completed without opening the window.
6. In Windows Settings → System → Notifications, "Looty" is listed with its name and icon (not "app.looty.desktop").

## Taskbar (notification area)

7. The Looty icon is in the notification area (possibly under the ^ arrow). Hover: the tooltip says "Looty".
8. Right-click it: the menu shows **Open Looty**, **Inbox**, **New task…**, the **Overdue** and **Today** sections with your tasks (each with **Open** and **Mark as done**) and **Launch at login**.
9. Close the window with the X: Looty keeps running in the notification area. **Open Looty** brings it back.
10. Tick **Launch at login**, sign out of Windows and back in. Looty starts hidden in the notification area. Untick it again.

## Shortcuts and links

11. With the Looty window in the background, press **Ctrl+Alt+N**: the quick-add window opens. Add a task.
12. Press **Ctrl+Alt+L**: the Looty window comes to the front.
13. In a browser, open a document link `looty://w/<space>/doc/<page>` (or "Open in the app" from Looty on the web).
    The browser asks to open Looty; the document opens in the app.

## Files

14. Download a file from a document or the drive. Looty asks where to save it (unless you turned this off in Settings), and the file opens correctly.

## Uninstall

15. Settings → Apps → Looty → Uninstall. Looty disappears from the Start menu and the notification area. A `looty://` link no longer opens anything.
