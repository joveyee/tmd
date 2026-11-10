use crate::error::AppError;
use std::path::Path;

/// Extensible binary document scraper (PDF, DOCX).
///
/// Supports conditional compilation via `--features pdf`, `--features docx`, or `--features full`.
pub fn extract_document_text(path: &Path) -> Result<String, AppError> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    match ext.to_ascii_lowercase().as_str() {
        #[cfg(feature = "pdf")]
        "pdf" => pdf_extract::extract_text(path)
            .map_err(|e| AppError::Processing(format!("PDF extraction failed: {e}"))),

        #[cfg(not(feature = "pdf"))]
        "pdf" => Err(AppError::Processing(format!(
            "PDF support is disabled in this build for '{}'. Recompile with `--features pdf` or `--features full`.",
            path.display()
        ))),

        #[cfg(feature = "docx")]
        "docx" => docx_lite::extract_text(path)
            .map_err(|e| AppError::Processing(format!("DOCX extraction failed: {e}"))),

        #[cfg(not(feature = "docx"))]
        "docx" => Err(AppError::Processing(format!(
            "DOCX support is disabled in this build for '{}'. Recompile with `--features docx` or `--features full`.",
            path.display()
        ))),

        _ => Err(AppError::Processing(format!(
            "Unsupported binary document format for '{}'",
            path.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_binary_format() {
        let path = Path::new("file.xyz");
        let result = extract_document_text(path);
        assert!(result.is_err());
    }

    #[cfg(not(feature = "pdf"))]
    #[test]
    fn test_pdf_disabled_message() {
        let path = Path::new("test.pdf");
        let result = extract_document_text(path);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("PDF support is disabled"));
    }

    #[cfg(not(feature = "docx"))]
    #[test]
    fn test_docx_disabled_message() {
        let path = Path::new("test.docx");
        let result = extract_document_text(path);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("DOCX support is disabled"));
    }
}
