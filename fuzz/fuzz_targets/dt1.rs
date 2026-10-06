#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(d) = d2_formats::dt1::Dt1::parse(data) {
        let _ = d;
    }
});
