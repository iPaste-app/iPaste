use serde::Serialize;

/// Stable error codes let the UI offer an action without parsing diagnostic text.
#[derive(Debug, Serialize)]
#[serde(tag = "code", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ImageOcrError {
    ModelsMissing { missing_files: Vec<String> },
    ModelsIncomplete { missing_files: Vec<String> },
    RecognitionFailed { message: String },
}

impl From<String> for ImageOcrError {
    fn from(message: String) -> Self {
        Self::RecognitionFailed { message }
    }
}
