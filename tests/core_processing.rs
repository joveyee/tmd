use tokens_md_rs::core::converter::ContentOptimizer;
use tokens_md_rs::core::parsers::text::clean_text_slice;
use tokens_md_rs::core::token_estimator::TokenEstimator;

#[test]
fn test_boilerplate_and_duplicate_stripping() {
    let raw = r#"
    Page 1 of 10
    This is an important system architecture note.
    ======================
    Page 2 of 10
    This is an important system architecture note.
    
    This is unique trailing commentary.
    "#;

    let cleaned = clean_text_slice(raw);
    let mut optimizer = ContentOptimizer::new();
    let result = optimizer.optimize(&cleaned);

    // Verify duplicate paragraph collapsed to 1 occurrence
    let count = result
        .matches("This is an important system architecture note.")
        .count();
    assert_eq!(count, 1);
    assert!(!result.contains("Page 1 of 10"));
}

#[test]
fn test_token_reduction_calculation() {
    let original = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(20);
    let optimized = "Lorem ipsum dolor sit amet, consectetur adipiscing elit.";

    let estimator = TokenEstimator::new().expect("Failed to initialize BPE");
    let metrics = estimator.compute_metrics(&original, &optimized);

    assert!(metrics.savings_percentage > 80.0);
    assert!(metrics.raw_tokens > metrics.optimized_tokens);
}
