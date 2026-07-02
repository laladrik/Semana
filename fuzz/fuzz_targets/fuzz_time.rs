#![no_main]

use libfuzzer_sys::fuzz_target;
use std::str::FromStr;

fuzz_target!(|data: &str| {
    let _ = calendar::date::Time::from_str(data);
});
