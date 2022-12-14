use std::{
    env::{self, VarError},
    path::Path,
};

use thiserror::Error;

/// Gets an environment variable for the given key
pub fn get(key: &str) -> Result<String, VarError> {
    env::var(key)
}

/// Sets an environment variable to the given key and value
pub fn set(key: &str, value: &str) {
    env::set_var(key, value)
}

/// Tries to load a variable from the shell env and if not found returns the provided default value
pub fn get_or_default(key: &str, default: &str) -> String {
    get(key).unwrap_or_else(|_| String::from(default))
}

/// Retrieves a vec of values for the given keys set in the env, in the order
/// of the input vec. Panics if the requested variables are not set.
pub fn get_multiple(keys: &[&str]) -> Vec<String> {
    let mut results = vec![];
    for key in keys {
        match env::var(key) {
            Ok(value) => {
                results.push(value);
            }
            Err(e) => panic!("Error at key {}, {}", key, e),
        };
    }
    results
}

/// Retrieves a vec of values for the given keys set in the env, in the order
/// of the input vec, default to the given value if not found.
pub fn get_or_default_multiple(keys: &[(&str, &str)]) -> Vec<String> {
    let mut results = vec![];
    for (key, default) in keys {
        match env::var(key) {
            Ok(value) => {
                results.push(value);
            }
            Err(_) => results.push(default.to_string()),
        };
    }
    results
}

/// Reads a file and sets all of its declared variables in the shell environment
pub fn load_from_file(path: &str) -> Result<(), ConfigError> {
    dotenv::from_path(Path::new(path)).map_err(ConfigError::DotEnv)
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error while loading .env: {0}")]
    DotEnv(#[from] dotenv::Error),
}
