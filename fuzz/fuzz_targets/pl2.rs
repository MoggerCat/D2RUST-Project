#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = d2_formats::palette::Pl2::parse(data);
});
