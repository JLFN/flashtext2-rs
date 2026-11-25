//! Edge-case tests for the trie walk and the public surface.
//!
//! `test_keyword_processor.rs` covers the 49-case conformance fixture, the
//! replace path, length bookkeeping and the static-keyword lifetime split.
//! This file pins the behaviours that fixture does not exercise directly:
//! how a failed traversal backtracks, what a token boundary is, how the two
//! matching modes differ, and the exact unit of the span offsets.
//!
//! Every expectation was read off the implementation in `src/shared.rs` and
//! confirmed by running it on this tree.

use flashtext2::{case_insensitive, case_sensitive};

// ---------------------------------------------------------------------------
// Matching modes
// ---------------------------------------------------------------------------

#[test]
fn case_sensitive_and_insensitive_disagree_only_on_case() {
    let mut strict = case_sensitive::KeywordProcessor::new();
    strict.add_keywords_from_iter(["Rust", "TOR"]);

    let mut loose = case_insensitive::KeywordProcessor::new();
    loose.add_keywords_from_iter(["Rust", "TOR"]);

    assert_eq!(
        strict.extract_keywords("Rust tor").collect::<Vec<_>>(),
        ["Rust"]
    );
    assert_eq!(
        loose.extract_keywords("Rust tor").collect::<Vec<_>>(),
        ["Rust", "TOR"]
    );
}

#[test]
fn case_insensitive_reports_the_stored_keyword_not_the_text_spelling() {
    let mut kp = case_insensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["Rust"]);

    // The extracted item borrows from the trie, so it is the registered
    // spelling even when the input differs.
    assert_eq!(
        kp.extract_keywords("RUST rust RuSt").collect::<Vec<_>>(),
        ["Rust", "Rust", "Rust"]
    );
}

// ---------------------------------------------------------------------------
// Tokenisation (Unicode word bounds)
// ---------------------------------------------------------------------------

#[test]
fn punctuation_is_part_of_its_own_token_sequence() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["hello", "hello!"]);

    assert_eq!(
        kp.extract_keywords_with_span("say hello! now")
            .collect::<Vec<_>>(),
        [("hello!", 4, 10)]
    );

    let mut bare = case_sensitive::KeywordProcessor::new();
    bare.add_keyword("hello");
    assert_eq!(
        bare.extract_keywords_with_span("say hello. now")
            .collect::<Vec<_>>(),
        [("hello", 4, 9)]
    );
}

#[test]
fn text_without_a_boundary_is_one_token() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["aa", "bb"]);

    assert!(kp.extract_keywords("aabb").next().is_none());
    assert_eq!(
        kp.extract_keywords("aa bb").collect::<Vec<_>>(),
        ["aa", "bb"]
    );
}

#[test]
fn a_keyword_does_not_match_inside_a_longer_word() {
    let mut kp = case_insensitive::KeywordProcessor::new();
    kp.add_keyword("ruby");

    assert!(kp.extract_keywords("rubyist").next().is_none());
    assert_eq!(
        kp.extract_keywords("a ruby gem").collect::<Vec<_>>(),
        ["ruby"]
    );
}

// ---------------------------------------------------------------------------
// Trie walk: longest match and backtracking
// ---------------------------------------------------------------------------

#[test]
fn the_longest_registered_sequence_wins() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["New York", "New York City", "York"]);

    assert_eq!(
        kp.extract_keywords_with_span("I live in New York City now")
            .collect::<Vec<_>>(),
        [("New York City", 10, 23)]
    );
    assert_eq!(
        kp.extract_keywords("I live in New York now")
            .collect::<Vec<_>>(),
        ["New York"]
    );
}

#[test]
fn a_dead_end_backtracks_instead_of_swallowing_the_rest_of_the_text() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["New York"]);

    // "New" starts the sequence, "things" breaks it. The iterator must resume
    // at the token after "New"; if it did not, "York" could never match later.
    assert!(kp.extract_keywords("I like New things").next().is_none());
    assert_eq!(
        kp.extract_keywords("New things, then New York")
            .collect::<Vec<_>>(),
        ["New York"]
    );
}

#[test]
fn a_longer_word_continuing_a_prefix_keyword_is_not_a_match() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["art", "artist"]);

    assert_eq!(
        kp.extract_keywords("the art show").collect::<Vec<_>>(),
        ["art"]
    );
    assert_eq!(
        kp.extract_keywords("the artist").collect::<Vec<_>>(),
        ["artist"]
    );
    // "artistry" is a different token, so neither keyword matches; the walk
    // must not fall back to the shorter "art".
    assert!(kp.extract_keywords("artistry").next().is_none());
}

#[test]
fn every_occurrence_is_yielded_once() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keyword("rust");

    assert_eq!(
        kp.extract_keywords_with_span("rust and rust and rust")
            .collect::<Vec<_>>(),
        [("rust", 0, 4), ("rust", 9, 13), ("rust", 18, 22)]
    );
}

// ---------------------------------------------------------------------------
// Span unit
// ---------------------------------------------------------------------------

#[test]
fn spans_are_byte_offsets_even_for_non_ascii_text() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["café", "straße"]);

    let text = "Le café, die straße";
    let hits: Vec<(&str, usize, usize)> = kp.extract_keywords_with_span(text).collect();

    // The implementation builds spans from `split_word_bound_indices`, which
    // reports byte indices. For "café" the character range is 3..7 but the
    // byte range is 3..8, because é is two bytes.
    assert_eq!(hits, [("café", 3, 8), ("straße", 14, 21)]);
    assert_eq!(text.len(), 21);
    assert_eq!(text.chars().count(), 19);

    for (keyword, start, end) in hits {
        assert_eq!(&text[start..end], keyword);
    }
}

#[test]
fn ascii_spans_slice_the_input_directly() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["Hello", "Rust"]);

    let text = "Hello, I love Rust!";
    let hits: Vec<(&str, usize, usize)> = kp.extract_keywords_with_span(text).collect();

    assert_eq!(hits, [("Hello", 0, 5), ("Rust", 14, 18)]);
    for (keyword, start, end) in hits {
        assert_eq!(&text[start..end], keyword);
    }
}

#[test]
fn spans_are_ordered_and_within_bounds() {
    let mut kp = case_insensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["one", "two", "three"]);

    let text = "one, two and three";
    let mut previous_end = 0;
    let mut count = 0;
    for (_, start, end) in kp.extract_keywords_with_span(text) {
        assert!(start < end);
        assert!(end <= text.len());
        assert!(start >= previous_end, "spans are yielded in input order");
        previous_end = end;
        count += 1;
    }
    assert_eq!(count, 3);
}

// ---------------------------------------------------------------------------
// Replacement
// ---------------------------------------------------------------------------

#[test]
fn replacement_of_a_different_length_does_not_shift_later_spans() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keyword_with_clean_word("a", "AAA");
    kp.add_keyword_with_clean_word("b", "B");

    // The clean word is longer than the keyword, so a naive implementation
    // would corrupt the second replacement.
    assert_eq!(kp.replace_keywords("a b a"), "AAA B AAA");
}

#[test]
fn replacement_keeps_text_outside_the_matches() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keyword_with_clean_word("Hello", "Hey");

    assert_eq!(kp.replace_keywords("Hello!"), "Hey!");
    assert_eq!(kp.replace_keywords("say Hello now"), "say Hey now");
    assert_eq!(kp.replace_keywords("nothing here"), "nothing here");
    assert_eq!(kp.replace_keywords(""), "");
}

// ---------------------------------------------------------------------------
// Empty states
// ---------------------------------------------------------------------------

#[test]
fn an_empty_processor_yields_nothing_and_replaces_nothing() {
    let kp = case_sensitive::KeywordProcessor::new();

    assert_eq!(kp.len(), 0);
    assert!(kp.is_empty());
    assert!(kp.extract_keywords("any text").next().is_none());
    assert!(kp.extract_keywords_with_span("any text").next().is_none());
    assert_eq!(kp.replace_keywords("any text"), "any text");
}

#[test]
fn an_empty_text_yields_nothing() {
    let mut kp = case_insensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["rust"]);

    assert!(kp.extract_keywords("").next().is_none());
    assert!(kp.extract_keywords_with_span("").next().is_none());
    assert_eq!(kp.replace_keywords(""), "");
}

// ---------------------------------------------------------------------------
// Length semantics
// ---------------------------------------------------------------------------

#[test]
fn len_counts_distinct_keywords_and_overwrites_do_not_grow_it() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    assert_eq!(kp.len(), 0);

    kp.add_keyword("Hello");
    kp.add_keyword("Hello");
    assert_eq!(kp.len(), 1, "a repeated keyword is not a new keyword");

    kp.add_keyword_with_clean_word("Hello", "Hi");
    assert_eq!(
        kp.len(),
        1,
        "overriding the clean word is not a new keyword"
    );
    assert_eq!(kp.replace_keywords("Hello"), "Hi");

    kp.add_keywords_from_iter(["Rust", "world"]);
    assert_eq!(kp.len(), 3);
    assert!(!kp.is_empty());
}

#[test]
fn keywords_with_a_shared_prefix_are_counted_separately() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["New", "New York", "New York City"]);
    assert_eq!(kp.len(), 3);
}

// ---------------------------------------------------------------------------
// Iteration contract
// ---------------------------------------------------------------------------

#[test]
fn the_iterator_can_be_partially_consumed() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["a", "b", "c"]);

    let mut it = kp.extract_keywords("a b c");
    assert_eq!(it.next(), Some("a"));
    assert_eq!(it.next(), Some("b"));
    drop(it);
}

#[test]
fn size_hint_bounds_the_lexeme_count() {
    let mut kp = case_sensitive::KeywordProcessor::new();
    kp.add_keywords_from_iter(["one", "two"]);

    let text = "one two three four";
    let it = kp.extract_keywords(text);
    let (lower, upper) = it.size_hint();

    // The extractor collects `split_word_bound_indices` up front, so the upper
    // bound is the lexeme count of the input. "one two three four" splits into
    // one / space / two / space / three / space / four, i.e. seven lexemes.
    let lexemes = 7;
    assert_eq!(lower, 0);
    assert_eq!(upper, Some(lexemes));
}
