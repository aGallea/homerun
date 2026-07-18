import { open } from "@tauri-apps/plugin-shell";

/**
 * Opens a URL in the user's default system browser.
 *
 * A plain `<a target="_blank">` does nothing inside the Tauri/WKWebView
 * webview — the webview has no notion of "open a new browser window" — so
 * external links must be routed through the shell plugin's `open` instead.
 */
export async function openExternal(url: string): Promise<void> {
  try {
    await open(url);
  } catch (e) {
    console.error(`Failed to open external URL ${url}:`, e);
  }
}
