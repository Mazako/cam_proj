use std::env;

use subtle::ConstantTimeEq;
use thiserror::Error;

const LOGIN_ENV: &str = "CAMWATCH_USER_LOGIN";
const PASSWORD_ENV: &str = "CAMWATCH_USER_PASSWORD";

#[derive(Error, Debug, PartialEq, Eq)]
pub enum AuthConfigError {
    #[error("{LOGIN_ENV} and {PASSWORD_ENV} must be set together")]
    Incomplete,
    #[error("{0} cannot be empty")]
    EmptyValue(&'static str),
    #[error("{0} must contain valid UTF-8")]
    InvalidUnicode(&'static str),
}

pub struct AuthService {
    login: String,
    password: String,
}

impl AuthService {
    pub fn from_environment() -> Result<Self, AuthConfigError> {
        let login = read_environment_variable(LOGIN_ENV)?;
        let password = read_environment_variable(PASSWORD_ENV)?;

        match (login, password) {
            (None, None) => Ok(Self::from_values("admin", "admin")),
            (Some(login), Some(password)) => Ok(Self::from_values(&login, &password)),
            _ => Err(AuthConfigError::Incomplete),
        }
    }

    pub fn verify(&self, login: &str, password: &str) -> bool {
        let login_matches = login.as_bytes().ct_eq(self.login.as_bytes());
        let password_matches = password.as_bytes().ct_eq(self.password.as_bytes());
        (login_matches & password_matches).into()
    }

    fn from_values(login: &str, password: &str) -> Self {
        Self {
            login: login.to_owned(),
            password: password.to_owned(),
        }
    }
}

fn read_environment_variable(name: &'static str) -> Result<Option<String>, AuthConfigError> {
    match env::var(name) {
        Ok(value) if value.is_empty() => Err(AuthConfigError::EmptyValue(name)),
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(AuthConfigError::InvalidUnicode(name)),
    }
}
