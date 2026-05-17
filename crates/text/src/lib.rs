#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSegment {
    pub text: String,
}

pub fn clean_ocr_text(input: &str) -> String {
    input
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn split_segments(input: &str) -> Vec<TextSegment> {
    clean_ocr_text(input)
        .lines()
        .map(|line| TextSegment {
            text: line.to_owned(),
        })
        .collect()
}
