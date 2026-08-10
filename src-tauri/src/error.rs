//! Error model for the application.
//!
//! SPEC.md section 46 requires that every error is classified and that raw Rust
//! errors never become the primary user-facing message. `AppError` therefore
//! serializes to a payload carrying a short title, a plain-language message, a
//! category, and optional technical details the UI can hide behind a disclosure.

use serde::Serialize;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCategory {
    UserActionable,
    Configuration,
    ContentValidation,
    ExternalProcess,
    Filesystem,
    Database,
    Internal,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    UserActionable { title: String, message: String },

    #[error("{message}")]
    Configuration { title: String, message: String },

    #[error("archive could not be read: {path}")]
    ArchiveUnreadable { path: String, detail: String },

    #[error("filesystem error at {path}")]
    Filesystem {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("database error")]
    Database(#[from] sqlx::Error),

    #[error("database migration error")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("serialization error")]
    Serialization(#[from] serde_json::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn user(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::UserActionable {
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn config(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Configuration {
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::UserActionable { .. } => ErrorCategory::UserActionable,
            Self::Configuration { .. } => ErrorCategory::Configuration,
            Self::ArchiveUnreadable { .. } => ErrorCategory::ContentValidation,
            Self::Filesystem { .. } => ErrorCategory::Filesystem,
            Self::Database(_) | Self::Migration(_) => ErrorCategory::Database,
            Self::Serialization(_) | Self::Internal(_) => ErrorCategory::Internal,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::UserActionable { title, .. } | Self::Configuration { title, .. } => title.clone(),
            Self::ArchiveUnreadable { .. } => "Unable to read archive".into(),
            Self::Filesystem { .. } => "Unable to access a file or folder".into(),
            Self::Database(_) | Self::Migration(_) => "Library database problem".into(),
            Self::Serialization(_) | Self::Internal(_) => "Unexpected problem".into(),
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::UserActionable { message, .. } | Self::Configuration { message, .. } => {
                message.clone()
            }
            Self::ArchiveUnreadable { path, .. } => format!(
                "The archive could not be opened. The file may be damaged or incomplete.\n{path}"
            ),
            Self::Filesystem { path, .. } => {
                format!("The location could not be accessed.\n{path}")
            }
            Self::Database(_) | Self::Migration(_) => {
                "The local library database could not be read or updated.".into()
            }
            Self::Serialization(_) | Self::Internal(_) => {
                "Something went wrong inside the application. No ROM files were changed.".into()
            }
        }
    }

    /// The raw error text, shown only behind an expandable "technical details"
    /// disclosure so it never becomes the primary message.
    pub fn technical_details(&self) -> Option<String> {
        match self {
            Self::UserActionable { .. } | Self::Configuration { .. } => None,
            Self::ArchiveUnreadable { detail, .. } => Some(detail.clone()),
            Self::Filesystem { source, .. } => Some(source.to_string()),
            Self::Database(e) => Some(e.to_string()),
            Self::Migration(e) => Some(e.to_string()),
            Self::Serialization(e) => Some(e.to_string()),
            Self::Internal(e) => Some(e.clone()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorPayload {
    pub category: ErrorCategory,
    pub title: String,
    pub message: String,
    pub technical_details: Option<String>,
}

impl From<&AppError> for AppErrorPayload {
    fn from(error: &AppError) -> Self {
        Self {
            category: error.category(),
            title: error.title(),
            message: error.message(),
            technical_details: error.technical_details(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        AppErrorPayload::from(self).serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_map_to_spec_buckets() {
        assert_eq!(
            AppError::user("t", "m").category(),
            ErrorCategory::UserActionable
        );
        assert_eq!(
            AppError::ArchiveUnreadable {
                path: "a.zip".into(),
                detail: "invalid Zip archive".into(),
            }
            .category(),
            ErrorCategory::ContentValidation
        );
        assert_eq!(
            AppError::internal("boom").category(),
            ErrorCategory::Internal
        );
    }

    #[test]
    fn raw_error_text_is_kept_out_of_the_primary_message() {
        let error = AppError::ArchiveUnreadable {
            path: "D:\\roms\\abc.zip".into(),
            detail: "invalid Zip archive: Could not find central directory end".into(),
        };

        assert!(!error.message().contains("central directory"));
        assert!(error
            .technical_details()
            .unwrap()
            .contains("central directory"));
    }

    #[test]
    fn user_errors_have_no_technical_details() {
        assert!(AppError::user("Title", "Message").technical_details().is_none());
    }
}
