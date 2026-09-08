//! Mbongo Compute Confidential Authorization (`confidential-auth-v1`), run
//! against the reference implementation with the **REFERENCE / TEST ONLY**
//! attestation verifier.
//!
//! Prints the report and exits non-zero unless every case passes. This is
//! the named CI gate; a future hardware adapter runs the same suite
//! through its own [`AttestationVerifier`].
//!
//! [`AttestationVerifier`]: mbongo_compute::confidential::AttestationVerifier

use mbongo_compute::confidential::reference::{ReferenceAttestationVerifier, REFERENCE_LABEL};
use mbongo_compute::confidential::suite::run_all;

#[tokio::main]
async fn main() {
    let report = run_all(|| Box::new(ReferenceAttestationVerifier)).await;
    print!("{}", report.render());
    println!("LABEL: {REFERENCE_LABEL}");
    if report.passed() {
        println!("CONFIDENTIAL AUTHORIZATION: PASS");
    } else {
        println!("CONFIDENTIAL AUTHORIZATION: FAIL");
        std::process::exit(1);
    }
}
