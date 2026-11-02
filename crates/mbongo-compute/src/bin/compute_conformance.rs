//! Mbongo Compute Conformance, run against the reference implementation.
//!
//! Prints the machine-readable report and exits non-zero unless every
//! case passes. This is the named CI gate; a future implementation runs
//! the same suite through its own [`Subject`] adapter.
//!
//! [`Subject`]: mbongo_compute::conformance::Subject

use mbongo_compute::conformance::reference::ReferenceSubject;
use mbongo_compute::conformance::{run_all, AUTHORITIES};

#[tokio::main]
async fn main() {
    let report = run_all(ReferenceSubject::new).await;
    print!("{}", report.render());
    println!("AUTHORITIES:");
    for a in AUTHORITIES {
        println!("  {a}");
    }
    if !report.passed() {
        std::process::exit(1);
    }
}
