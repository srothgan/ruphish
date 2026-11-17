#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| ruphish_fuzz::routing::check(data));
