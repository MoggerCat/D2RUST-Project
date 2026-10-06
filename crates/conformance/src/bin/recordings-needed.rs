//! Prints the recordings the conformance harnesses wait for, with the
//! recorder invocations to queue (`conformance::needs`).

fn main() {
    print!("{}", conformance::needs::report());
}
