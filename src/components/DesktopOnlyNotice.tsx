import "./DesktopOnlyNotice.css";

/**
 * Shown when the frontend is loaded outside the Tauri window.
 *
 * `npm run tauri dev` starts a Vite server on port 1420 purely so the desktop
 * window has something to load. Opening that URL in a browser gives a page with
 * no backend behind it, because `invoke` only exists inside the webview Tauri
 * creates. Without this notice every command fails with an opaque
 * "cannot read properties of undefined" error.
 */
export function DesktopOnlyNotice() {
  return (
    <main className="desktop-only">
      <div className="desktop-only-card">
        <p className="desktop-only-eyebrow">Aeonic Arcadia</p>
        <h1>Open the desktop window instead</h1>
        <p>
          This page is the development server for the interface only. The
          scanner, the library database, and every other capability live in the
          desktop application, which this browser tab has no connection to.
        </p>
        <p>Run the app from the project folder:</p>
        <pre>npm run tauri dev</pre>
        <p className="desktop-only-hint">
          A separate window titled &ldquo;Aeonic Arcadia&rdquo; will open. Use
          that window; you can close this tab.
        </p>
      </div>
    </main>
  );
}
