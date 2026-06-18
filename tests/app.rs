//! Integration tests for the app-level pure helpers.

use solfig::app::{amount_within_balance, is_subsequence, url_encode, wrap_index, AIRDROP_STEPS};

#[test]
fn is_subsequence_matches_in_order() {
    assert!(is_subsequence("abc", "axbxc"));
    assert!(is_subsequence("", "anything")); // empty needle always matches
    assert!(is_subsequence("dev", "api.devnet.solana.com"));
    assert!(!is_subsequence("cba", "axbxc")); // wrong order
    assert!(!is_subsequence("abcd", "abc")); // needle longer than match
}

#[test]
fn url_encode_preserves_unreserved_and_escapes_the_rest() {
    // RFC 3986 unreserved chars pass through untouched.
    assert_eq!(url_encode("AZaz09-_.~"), "AZaz09-_.~");
    // Reserved / unsafe chars are percent-encoded (uppercase hex).
    assert_eq!(url_encode("a b"), "a%20b");
    assert_eq!(
        url_encode("http://h:8899/p?x=1&y=2"),
        "http%3A%2F%2Fh%3A8899%2Fp%3Fx%3D1%26y%3D2"
    );
}

#[test]
fn airdrop_steps_are_sorted_and_positive() {
    assert!(AIRDROP_STEPS.windows(2).all(|w| w[0] < w[1]));
    assert!(AIRDROP_STEPS.iter().all(|&s| s > 0.0));
}

#[test]
fn amount_within_balance_caps_at_known_balance() {
    let two_sol = Some(2_000_000_000u64);
    // Under and exactly at the balance are allowed; over is rejected.
    assert!(amount_within_balance(two_sol, "1"));
    assert!(amount_within_balance(two_sol, "1.5"));
    assert!(amount_within_balance(two_sol, "2")); // boundary is inclusive
    assert!(!amount_within_balance(two_sol, "2.5"));
    assert!(!amount_within_balance(two_sol, "3"));

    // Unknown balance is permissive (can't validate yet).
    assert!(amount_within_balance(None, "999999"));

    // In-progress / non-numeric input is permissive so typing isn't blocked.
    assert!(amount_within_balance(two_sol, ""));
    assert!(amount_within_balance(two_sol, "abc"));

    // A zero balance rejects any positive amount.
    assert!(amount_within_balance(Some(0), "0"));
    assert!(!amount_within_balance(Some(0), "0.0001"));
}

#[test]
fn wrap_index_wraps_both_ends() {
    // Forward within range and wrapping past the end back to 0.
    assert_eq!(wrap_index(0, 1, 3), 1);
    assert_eq!(wrap_index(2, 1, 3), 0);
    // Backward wrapping past the start to the last element.
    assert_eq!(wrap_index(0, -1, 3), 2);
    // An empty list has no valid index.
    assert_eq!(wrap_index(0, 1, 0), 0);
}
