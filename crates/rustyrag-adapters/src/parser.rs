use async_trait::async_trait;
use rustyrag_core::{Document, Error, ParserAdapter, RawDocument, Result};
use std::path::Path;

pub struct AutoParser;

#[async_trait]
impl ParserAdapter for AutoParser {
    async fn parse(&self, raw: &RawDocument) -> Result<Document> {
        let parsed_content = if is_pdf(raw) {
            parse_pdf(&raw.uri)?
        } else if is_html(raw) {
            parse_html(&raw.content)
        } else {
            raw.content.clone()
        };

        Ok(Document {
            raw: raw.clone(),
            parsed_content,
        })
    }
}

fn is_pdf(raw: &RawDocument) -> bool {
    has_extension(raw, "pdf")
}

fn is_html(raw: &RawDocument) -> bool {
    has_extension(raw, "html") || has_extension(raw, "htm")
}

fn has_extension(raw: &RawDocument, ext: &str) -> bool {
    Path::new(&raw.uri)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn parse_pdf(uri: &str) -> Result<String> {
    pdf_extract::extract_text(uri).map_err(|err| Error::Adapter {
        adapter: "auto".into(),
        message: format!("pdf extract failed for {uri}: {err}"),
    })
}

fn parse_html(content: &str) -> String {
    html2text::from_read(content.as_bytes(), 80)
}
