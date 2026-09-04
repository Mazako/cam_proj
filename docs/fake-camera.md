# Lokalna fake camera

Repozytorium zawiera compose uruchamiający MediaMTX oraz FFmpeg. FFmpeg generuje obraz testowy `testsrc2` w rozdzielczości 640x360 przy 10 FPS i publikuje go jako H.264 po RTSP.

Uruchom stream:

```sh
docker compose up -d
```

Stream jest dostępny dla aplikacji uruchomionej na hoście pod adresem:

```sh
export CAMWATCH_FAKE_CAMERA_RTSP_URL=rtsp://127.0.0.1:8554/fake-camera
```

Następnie uruchom Camwatch w drugim terminalu:

```sh
cargo run -p camwatch-server -- --config config/camwatch.fake-camera.toml
```

Panel będzie dostępny pod `http://127.0.0.1:8080`. Domyślne dane logowania lokalnego panelu to `admin` / `admin`, jeśli nie ustawisz `CAMWATCH_USER_LOGIN` i `CAMWATCH_USER_PASSWORD`.

Logi i zatrzymanie środowiska:

```sh
docker compose logs -f fake-camera
docker compose down
```

MediaMTX używa wyłącznie transportu RTSP TCP, dzięki czemu publikowanie przez sieć compose i odczyt z hosta nie wymagają mapowania portów UDP.
