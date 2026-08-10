import { useState } from "react";

import type { AppErrorPayload } from "../types/api";
import "./ErrorBanner.css";

interface Props {
  error: AppErrorPayload;
  onDismiss: () => void;
}

/**
 * Presents an error the way SPEC.md section 46 requires: a short title, a
 * plain-language message, and the raw detail tucked behind a disclosure rather
 * than shown as the primary text.
 */
export function ErrorBanner({ error, onDismiss }: Props) {
  const [showDetails, setShowDetails] = useState(false);

  return (
    <div className="error-banner" role="alert">
      <div className="error-banner-body">
        <p className="error-banner-title">{error.title}</p>
        <p className="error-banner-message">{error.message}</p>

        {error.technicalDetails && (
          <>
            <button
              type="button"
              className="quiet error-banner-toggle"
              aria-expanded={showDetails}
              onClick={() => setShowDetails((value) => !value)}
            >
              {showDetails ? "Hide technical details" : "Technical details"}
            </button>
            {showDetails && (
              <pre className="error-banner-details">
                {error.technicalDetails}
              </pre>
            )}
          </>
        )}
      </div>

      <button
        type="button"
        className="quiet error-banner-dismiss"
        onClick={onDismiss}
        aria-label="Dismiss this message"
      >
        ✕
      </button>
    </div>
  );
}
