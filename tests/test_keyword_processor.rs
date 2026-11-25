//! Behavioural tests for both `KeywordProcessor` flavours.
//!
//! The conformance fixture `keyword_extractor_test_cases.json` holds 49 cases
//! covering word boundaries, overlapping keywords, multi-token phrases and
//! case handling. Each case carries the sentence, a keyword dictionary
//! (clean word -> surface forms) and the expected matches for the
//! case-insensitive and case-sensitive processors.

use flashtext2::{case_insensitive, case_sensitive};
use serde_json::Value;

const CASES: &str = include_str!("keyword_extractor_test_cases.json");

fn cases() -> Vec<Value> {
    serde_json::from_str::<Vec<Value>>(CASES).expect("fixture parses as a JSON array")
}

/// Flattens one case's keyword dictionary into (surface form, clean word)
/// pairs. No case declares the same surface form twice, so the pairs are
/// distinct and matcher order does not affect the result.
fn clean_word_pairs(case: &Value) -> Vec<(&str, &str)> {
    let dict = case["keyword_dict"]
        .as_object()
        .expect("case has a keyword_dict object");
    let mut pairs = Vec::new();
    for (clean_word, surfaces) in dict {
        for surface in surfaces.as_array().expect("surface form list") {
            pairs.push((
                surface.as_str().expect("surface form is a string"),
                clean_word.as_str(),
            ));
        }
    }
    pairs
}

fn expected<'a>(case: &'a Value, key: &str) -> Vec<&'a str> {
    case[key]
        .as_array()
        .unwrap_or_else(|| panic!("case has a {key} array"))
        .iter()
        .map(|v| v.as_str().expect("keyword is a string"))
        .collect()
}

/// Borrowed surface forms and clean words taken straight from the case, so no
/// copy is needed before building a processor.
fn sentence(case: &Value) -> &str {
    case["sentence"].as_str().expect("sentence is a string")
}

#[test]
fn test_case_insensitive_extraction() {
    for (id, case) in cases().iter().enumerate() {
        let pairs = clean_word_pairs(case);
        let mut kp = case_insensitive::KeywordProcessor::new();
        kp.add_keywords_with_clean_word_from_iter(pairs);
        let found: Vec<&str> = kp.extract_keywords(sentence(case)).collect();
        assert_eq!(
            found,
            expected(case, "keywords"),
            "case {id}: {}",
            case["explanation"]
        );
    }
}

#[test]
fn test_case_sensitive_extraction() {
    for (id, case) in cases().iter().enumerate() {
        let pairs = clean_word_pairs(case);
        let mut kp = case_sensitive::KeywordProcessor::new();
        kp.add_keywords_with_clean_word_from_iter(pairs);
        let found: Vec<&str> = kp.extract_keywords(sentence(case)).collect();
        assert_eq!(
            found,
            expected(case, "keywords_case_sensitive"),
            "case {id}: {}",
            case["explanation"]
        );
    }
}

#[test]
fn test_spans_cover_the_matched_keyword() {
    for (id, case) in cases().iter().enumerate() {
        let kp_source = clean_word_pairs(case);
        let surfaces: Vec<&str> = kp_source.iter().map(|(surface, _)| *surface).collect();
        let mut kp = case_insensitive::KeywordProcessor::new();
        kp.add_keywords_from_iter(surfaces.iter().copied());

        let text = sentence(case);
        let bytes: Vec<u8> = text.as_bytes().to_vec();
        for (keyword, start, end) in kp.extract_keywords_with_span(text) {
            assert!(
                text.get(start..end)
                    .is_some_and(|slice| slice.eq_ignore_ascii_case(keyword)),
                "case {id}: span [{start}, {end}) = {:?} does not match keyword {keyword:?} in {text:?}",
                text.get(start..end)
            );
            assert!(end <= bytes.len(), "case {id}: span end past input");
        }
    }
}

#[test]
fn test_replace_keywords_uses_clean_words() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keyword_with_clean_word("Hello", "Hey");
    kp.add_keyword_with_clean_word("love", "hate");
    kp.add_keyword_with_clean_word("Rust", "Java");

    let replaced = kp.replace_keywords("Hello, I love programming in Rust!");
    assert_eq!(replaced, "Hey, I hate programming in Java!");
}

#[test]
fn test_len_counts_distinct_keywords_and_allows_clean_word_override() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    assert!(kp.is_empty());

    kp.add_keyword("Hello");
    kp.add_keyword("Hello");
    assert_eq!(kp.len(), 1);

    kp.add_keyword_with_clean_word("Hello", "Hi");
    assert_eq!(kp.len(), 1, "overwriting a clean word is not a new keyword");
    assert_eq!(kp.replace_keywords("Hello"), "Hi");

    kp.add_keywords_from_iter(["Rust", "world"]);
    assert_eq!(kp.len(), 3);
    assert!(!kp.is_empty());
}

#[test]
fn test_case_insensitive_matches_across_case_and_unicode_folding() {
    let tokens = ["flour", "Maße", "ᾲ στο διάολο"];
    let mut kp = case_insensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(tokens);

    let text = "ﬂour, MASSE, ὰι στο διάολο";
    let found: Vec<&str> = kp.extract_keywords(text).collect();
    assert_eq!(found, tokens);
}

#[test]
fn test_case_sensitive_rejects_case_variants() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keyword("Hello");
    assert!(kp.extract_keywords("hello").next().is_none());
    assert_eq!(kp.extract_keywords("Hello").next(), Some("Hello"));
}

#[test]
fn test_longest_match_wins() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["New York", "New York City"]);
    let found: Vec<&str> = kp.extract_keywords("I live in New York City now").collect();
    assert_eq!(found, ["New York City"]);
}
