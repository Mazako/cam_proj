use std::io::{self, Read};

use camwatch::config::SecretManager;

fn main() {
    let manager = SecretManager::from_environment().unwrap_or_else(|error| {
        eprintln!("Unable to load encryption key: {error}");
        std::process::exit(1);
    });

    let mut value = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut value) {
        eprintln!("Unable to read plaintext: {error}");
        std::process::exit(1);
    }

    match manager.encrypt(&value) {
        Ok(ciphertext) => println!("{ciphertext}"),
        Err(error) => {
            eprintln!("Unable to encrypt plaintext: {error}");
            std::process::exit(1);
        }
    }
}
