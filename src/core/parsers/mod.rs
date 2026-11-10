pub mod html;
pub mod pdf_docx;
pub mod script_extractor;
pub mod text;

use crate::error::AppError;
use std::path::Path;

pub enum ContentType {
    Html,
    PlainText,
    BinaryDoc,
}

pub fn detect_type(path: &Path) -> ContentType {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" | "htm" | "xhtml" => ContentType::Html,
        "pdf" | "docx" => ContentType::BinaryDoc,
        _ => ContentType::PlainText,
    }
}

pub fn parse_file(path: &Path, content: &str) -> Result<String, AppError> {
    match detect_type(path) {
        ContentType::Html => html::extract_article_markdown(content),
        ContentType::PlainText => Ok(text::clean_text_slice(content)),
        ContentType::BinaryDoc => pdf_docx::extract_document_text(path),
    }
}
