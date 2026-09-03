use camwatch_server::auth::{AuthConfigError, AuthService};
use std::sync::{Mutex, OnceLock};

#[test]
fn compares_credentials_without_exposing_them() {
    let _lock = environment_lock()
        .lock()
        .expect("environment lock should work");
    let auth = AuthService::from_environment().expect("fallback should be available");

    assert!(auth.verify("admin", "admin"));
    assert!(!auth.verify("admin", "wrong"));
    assert!(!auth.verify("wrong", "admin"));
    assert!(!auth.verify("admin", "admin-longer"));
}

#[test]
fn environment_configuration_requires_two_non_empty_values() {
    let _lock = environment_lock()
        .lock()
        .expect("environment lock should work");
    let variable_names = ("CAMWATCH_USER_LOGIN", "CAMWATCH_USER_PASSWORD");
    unsafe {
        std::env::remove_var(variable_names.0);
        std::env::remove_var(variable_names.1);
    }

    let fallback = AuthService::from_environment().expect("fallback should be available");
    assert!(fallback.verify("admin", "admin"));

    unsafe {
        std::env::set_var(variable_names.0, "operator");
    }
    assert!(matches!(
        AuthService::from_environment(),
        Err(AuthConfigError::Incomplete)
    ));

    unsafe {
        std::env::set_var(variable_names.1, "");
    }
    assert!(matches!(
        AuthService::from_environment(),
        Err(AuthConfigError::EmptyValue("CAMWATCH_USER_PASSWORD"))
    ));

    unsafe {
        std::env::set_var(variable_names.1, "secret");
    }
    let configured = AuthService::from_environment().expect("configured auth should work");
    assert!(configured.verify("operator", "secret"));

    unsafe {
        std::env::remove_var(variable_names.0);
        std::env::remove_var(variable_names.1);
    }
}

fn environment_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
