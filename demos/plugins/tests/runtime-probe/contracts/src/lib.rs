//! Test-only ABI owner shared by independently linked host and plugin images.
#![no_std]
#![deny(missing_docs)]
include!(concat!(env!("OUT_DIR"), "/nc_tests_registry_probe.include.rs"));
