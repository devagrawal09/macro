use std::sync::Mutex;

use super::*;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct AllowedOriginsGuard(Option<std::ffi::OsString>);

impl Drop for AllowedOriginsGuard {
    fn drop(&mut self) {
        // SAFETY: this guard is dropped before the caller releases ENV_LOCK.
        unsafe {
            match self.0.take() {
                Some(value) => std::env::set_var("ALLOWED_ORIGINS", value),
                None => std::env::remove_var("ALLOWED_ORIGINS"),
            }
        }
    }
}

fn with_allowed_origins(value: Option<&str>, test: impl FnOnce()) {
    let _lock = ENV_LOCK.lock().expect("environment test lock poisoned");
    let _restore = AllowedOriginsGuard(std::env::var_os("ALLOWED_ORIGINS"));
    // SAFETY: every test in this crate that mutates ALLOWED_ORIGINS holds ENV_LOCK
    // for the mutation, assertion, and RAII restoration.
    unsafe {
        match value {
            Some(value) => std::env::set_var("ALLOWED_ORIGINS", value),
            None => std::env::remove_var("ALLOWED_ORIGINS"),
        }
    }
    test();
}

#[test]
fn allows_localhost_and_subdomain_localhost_dev_ports() {
    with_allowed_origins(None, || {
        for origin in [
            "http://localhost:3000",
            "http://localhost:3999",
            "http://localhost:20000",
            "http://alice.localhost:3000",
            "http://carol.localhost:3005",
        ] {
            assert!(is_allowed_origin(origin, false), "{origin}");
        }
    });
}

#[test]
fn rejects_non_local_and_out_of_range_origins() {
    with_allowed_origins(None, || {
        for origin in [
            "http://localhost:2999",
            "http://localhost:9000",
            "http://alice.localhost:9000",
            "https://alice.localhost:3000",
            "http://evil-localhost:3000",
            "http://alice.localhost.evil.com:3000",
            "http://example.com:3000",
        ] {
            assert!(!is_allowed_origin(origin, false), "{origin}");
        }
    });
}

#[test]
fn allows_static_origins() {
    with_allowed_origins(None, || {
        assert!(is_allowed_origin("https://macro.com", false));
        assert!(is_allowed_origin("tauri://localhost", false));
    });
}

#[test]
fn opaque_origin_requires_explicit_opt_in() {
    with_allowed_origins(None, || {
        assert!(!is_allowed_origin("null", false));
        assert!(is_allowed_origin("null", true));
    });
}

#[test]
fn configured_null_cannot_bypass_default_rejection() {
    with_allowed_origins(Some("null,https://configured.example"), || {
        assert!(!is_allowed_origin("null", false));
        assert!(is_allowed_origin("null", true));
        assert!(is_allowed_origin("https://configured.example", false));
        assert!(!is_allowed_origin("https://rejected.example", true));
    });
}
