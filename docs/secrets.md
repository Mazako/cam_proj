# Szyfrowana konfiguracja

Camwatch przechowuje wartości konfiguracyjne, które mogą zawierać sekrety, jako ciphertexty AES-256-GCM zakodowane Base64:

```text
enc:v1:aes256gcm:<base64(nonce || ciphertext || tag)>
```

Klucz ma 32 bajty i jest przekazywany aplikacji poza plikiem TOML:

```sh
export CAMWATCH_CONFIG_KEY="$(openssl rand -base64 32)"
```

Można też zapisać klucz do pliku ignorowanego przez Git i używać skryptów z repozytorium:

```sh
mkdir -p .local
./scripts/generate-secret-key.sh .local/camwatch.key
./scripts/encrypt-secret.sh --key .local/camwatch.key 'rtsp://127.0.0.1:8554/fake-camera'
```

Tekst można podać przez stdin, co nie umieszcza go w historii shella:

```sh
printf '%s' 'rtsp://127.0.0.1:8554/fake-camera' |
  ./scripts/encrypt-secret.sh --key .local/camwatch.key
```

Do lokalnego uruchamiania z VSCode skopiuj `.env.example` do `.env`, wpisz tam `CAMWATCH_CONFIG_KEY` i utwórz ignorowany plik `config/camwatch-local.toml`. Task `Camwatch: local` oraz konfiguracja debugowania używają tego pliku automatycznie.

Nie zapisuj `CAMWATCH_CONFIG_KEY` w repozytorium. Każda wartość powinna być szyfrowana ponownie przy zmianie, ponieważ nonce jest losowany dla każdego szyfrowania.

## Zasada działania

`Config::parse` parsuje strukturę TOML-u. Podczas bootstrapu `SecretManager` odszyfrowuje pola, a `AppState` przechowuje jego współdzieloną instancję. Runtime i klient R2 otrzymują odszyfrowane wartości tylko w pamięci. SQLite przechowuje ciphertexty, żeby restart aplikacji nie wymagał plaintextu w bazie.

## Dostosowanie testu

Test nie powinien ustawiać globalnych zmiennych środowiskowych z sekretem. Tworzymy stały klucz tylko dla testu:

```rust
use std::sync::Arc;

use camwatch::config::SecretManager;
use camwatch_server::app_state::bootstrap_with_secret_manager;

let secrets = Arc::new(SecretManager::from_key([9; 32]));
let state = bootstrap_with_secret_manager(config, secrets.clone())
    .await
    .expect("test bootstrap should succeed");
```

Fixture TOML-u musi zawierać ciphertext, a nie nazwę env-a:

```rust
let rtsp_url = secrets
    .encrypt("rtsp://127.0.0.1:8554/fake-camera")
    .expect("test secret should encrypt");

let config = Config::parse(&format!(
    r#"
[app]
bind_address = "127.0.0.1:8080"
database_path = "{}"
pre_event_seconds = 10
post_event_seconds = 20
rolling_buffer_seconds = 30

[[cameras]]
id = "fake-camera"
name = "Fake camera"
rtsp_url = "{}"
motion_min_area = 1000
yolo_confidence = 0.5
"#,
    database_path.display(),
    rtsp_url,
))
.expect("test configuration should parse");
```

Dla testów, które sprawdzają samą walidację konfiguracji, trzeba wykonać:

```rust
let config = config
    .decrypt_secrets(&secrets)
    .expect("test configuration should decrypt");
```

Można też sprawdzić, że sekret nie jest zapisany jawnie w SQLite:

```rust
assert!(camera.rtsp_url.starts_with("enc:v1:aes256gcm:"));
assert_eq!(secrets.decrypt(&camera.rtsp_url).unwrap(), expected_url);
```

Testy kryptograficzne powinny pokrywać szyfrowanie/deszyfrowanie, różne ciphertexty dla tego samego plaintextu oraz odrzucenie zmodyfikowanego ciphertextu.
