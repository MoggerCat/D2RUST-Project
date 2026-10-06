#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = d2_formats::dcc::Dcc::parse(data);
});
