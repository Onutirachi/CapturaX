//! A first and a second claim of the same instance name in one process
//! (contracts/single-instance.md).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use capturax_platform::{Answer, Claim, claim_instance};

/// A name no other test or running app uses.
fn unique(test: &str) -> String {
    format!("capturax-test-{}-{test}", std::process::id())
}

fn first(name: &str) -> capturax_platform::Listener {
    match claim_instance(name).unwrap() {
        Claim::First(listener) => listener,
        Claim::Running(_) => panic!("{name} is already taken"),
    }
}

fn running(name: &str) -> capturax_platform::Client {
    match claim_instance(name).unwrap() {
        Claim::First(_) => panic!("second claim of {name} got the name"),
        Claim::Running(client) => client,
    }
}

#[test]
fn second_claim_asks_the_first_to_show() {
    let name = unique("show");
    let shows = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&shows);
    let _serving = first(&name)
        .serve(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            true
        })
        .unwrap();

    let client = running(&name);
    assert_eq!(client.show(), Some(Answer::Raised));
    // The listener goes on serving later starts.
    assert_eq!(running(&name).show(), Some(Answer::Raised));
    assert_eq!(shows.load(Ordering::SeqCst), 2);
}

#[test]
fn answer_follows_the_callback() {
    let name = unique("not-raised");
    let _serving = first(&name).serve(|| false).unwrap();
    assert_eq!(running(&name).show(), Some(Answer::ShownNotRaised));
}

#[test]
fn a_claim_before_listening_already_holds_the_name() {
    let name = unique("held");
    let _listener = first(&name);
    let _ = running(&name);
}

#[test]
fn stopping_releases_the_name() {
    let name = unique("release");
    let serving = first(&name).serve(|| true).unwrap();
    drop(serving);
    let _again = first(&name);
}
