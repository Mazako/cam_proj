# Backlog spec-driven

Każdy task jest powiązany z wymaganiami z [dokumentu przewodniego](README.md). Task można uznać za ukończony dopiero po spełnieniu wszystkich kryteriów akceptacji oraz dodaniu testów właściwych dla jego warstwy.

## Zasady realizacji

- Implementujemy najpierw pionowy przepływ dla jednej kamery, a dopiero potem skalowanie do wielu kamer.
- Wszystkie operacje I/O są asynchroniczne lub wykonywane na wydzielonych workerach.
- Żaden sekret nie trafia do kodu, konfiguracji wersjonowanej ani logów.
- Każdy moduł ma jawne błędy domenowe i logi strukturalne.
- Elementy RTSP, ONVIF i Cloudflare R2 otrzymują adaptery, aby testy nie wymagały prawdziwej kamery ani dostępu do bucketa R2.

## Etap 0 — fundament projektu

### ARC-01: Utworzyć workspace Rust i bazową konfigurację

**Status:** Complete

**Pokrywa:** FR-01, NFR-03, NFR-04

**Zakres:** workspace Cargo, crate aplikacji, ładowanie konfiguracji TOML, walidacja konfiguracji oraz rozróżnienie danych jawnych i referencji sekretów.

**Kryteria akceptacji:**

- Aplikacja startuje z poprawnym plikiem konfiguracji.
- Nieprawidłowa konfiguracja kończy start czytelnym błędem bez ujawnienia sekretu.
- Konfiguracja może zawierać co najmniej jedną kamerę.

**Zależności:** brak.

### ARC-02: Dodać magazyn SQLite i migracje

**Pokrywa:** FR-06

**Zakres:** migracje `cameras` i `segments`; SQLite przechowuje wyłącznie dane potrzebne do bufora i inicjalizacji kamer. Historia zdarzeń oraz uploadów nie jest zapisywana.

**Kryteria akceptacji:**

- Migracje są idempotentne.
- Dane kamer i segmentów można zapisać oraz odczytać.
- Schemat nie zawiera tabel historii zdarzeń ani uploadów.

**Zależności:** ARC-01.

### ARC-03: Zdefiniować porty i adaptery infrastruktury

**Pokrywa:** wszystkie FR

**Zakres:** interfejsy `CameraStream`, `MotionDetector`, `PersonDetector`, `BucketUploader` i ich implementacje testowe oraz adaptery infrastruktury.

**Kryteria akceptacji:**

- Lifecycle klipu można uruchomić z fałszywą kamerą i fałszywym uploaderem.
- Kod domenowy nie importuje bezpośrednio GStreamera, ONVIF ani klienta S3/R2.

**Zależności:** ARC-01, ARC-02.

## Etap 1 — obraz i nagrywanie

### VID-01: Odbiór RTSP dla jednej kamery

**Status:** In progress

**Pokrywa:** FR-02, NFR-01

**Zakres:** pipeline GStreamer RTSP, wybór kodeka, zdarzenia stanu oraz reconnect z narastającym opóźnieniem.

**Kryteria akceptacji:**

- Aplikacja odbiera strumień Tapo przez co najmniej 30 minut.
- Po odłączeniu i ponownym podłączeniu kamery wraca do stanu online bez restartu procesu.
- Stan online/offline jest widoczny w logach i modelu aplikacji.

**Zależności:** ARC-01, ARC-03.

### VID-02: Zapis rotowanych segmentów MP4

**Pokrywa:** FR-05, NFR-02

**Zakres:** segmenty długości 1–2 sekundy, katalog per kamera, limit wieku i limit rozmiaru.

**Kryteria akceptacji:**

- Dla aktywnej kamery powstają odtwarzalne segmenty MP4.
- Przekroczenie limitu usuwa wyłącznie segmenty starsze od bufora.
- Restart aplikacji pozwala ponownie wykorzystać istniejące segmenty.

**Zależności:** VID-01.

### VID-03: Składanie klipu zdarzenia

**Pokrywa:** FR-05

**Zakres:** wybór segmentów z pre-bufferu, oczekiwanie na post-buffer i remuksowanie do pojedynczego MP4.

**Kryteria akceptacji:**

- Dla zdarzenia o znanym czasie wynik zawiera wymagany pre-buffer i post-buffer.
- Klip można odtworzyć w standardowym odtwarzaczu.
- Zdarzenia zachodzące na siebie rozszerzają jeden klip zamiast tworzyć duplikaty.

**Zależności:** ARC-02, VID-02.

## Etap 2 — detekcja i zdarzenia

### DET-01: MOG2 jako detektor ruchu

**Pokrywa:** FR-03

**Zakres:** przekazanie klatek z GStreamera do OpenCV, osobna instancja MOG2 per kamera, morfologia i próg powierzchni konturu.

**Kryteria akceptacji:**

- Nieruchoma scena po okresie nauki nie tworzy zdarzeń.
- Ruch większy od ustawionego progu generuje sygnał `MotionStarted`.
- Zmiana sceny po PTZ powoduje kontrolowany reset modelu tła.

**Zależności:** ARC-03, VID-01.

### DET-02: YOLO przez ONNX Runtime

**Pokrywa:** FR-04

**Zakres:** lokalny model ONNX, letterboxing, inferencja, odfiltrowanie klasy `person`, progi pewności oraz pula workerów.

**Kryteria akceptacji:**

- Klatka z osobą zwraca co najmniej jedną detekcję `person` z ramką i pewnością.
- Klatka bez osoby nie tworzy detekcji osoby powyżej ustawionego progu.
- Inferencja jest wykonywana tylko po sygnale ruchu i nie blokuje RTSP.

**Zależności:** ARC-03, DET-01.

### EVT-01: Silnik cyklu życia zdarzeń

**Pokrywa:** FR-03, FR-04, FR-05, FR-06

**Zakres:** in-memory lifecycle klipu, cooldown, rozszerzanie bieżącego klipu oraz przekazanie ukończonego klipu do kolejki uploadu. Żaden stan zdarzenia ani uploadu nie jest utrwalany.

**Kryteria akceptacji:**

- Ruch rozpoczyna bieżący klip, a YOLO wzbogaca jego dane o metadane osoby.
- Seria ruchów w oknie cooldown nie tworzy duplikatów.
- Zdarzenie przechodzi do finalizacji dopiero po ciszy wymaganej dla post-bufferu.

**Zależności:** ARC-02, VID-03, DET-01, DET-02.

## Etap 3 — integracje zewnętrzne

### ONV-01: Odczyt możliwości ONVIF i PTZ

**Pokrywa:** FR-01, FR-10

**Zakres:** połączenie ONVIF, odczyt możliwości PTZ przy starcie aplikacji oraz komendy `ContinuousMove` i `Stop` przez `OnvifConnection`.

**Kryteria akceptacji:**

- Aplikacja zna możliwości PTZ kamery od momentu jej uruchomienia.
- Ruch we wszystkich czterech kierunkach oraz zatrzymanie działa na obsługiwanej kamerze.
- Kamera bez PTZ nie powoduje błędu krytycznego.

**Zależności:** ARC-03.

### R2-01: Konfiguracja Cloudflare R2 i bucketa docelowego

**Pokrywa:** FR-07, NFR-03

**Zakres:** konfiguracja endpointu S3 R2, bucketa, prefiksu obiektów oraz bezpieczne wskazanie klucza dostępu i sekretu.

**Kryteria akceptacji:**

- Aplikacja potrafi zweryfikować dostęp do skonfigurowanego bucketa.
- Klucze dostępu nie są zapisywane w repozytorium, logach ani jawnej konfiguracji.
- Aplikacja odrzuca konfigurację bez endpointu, bucketa lub wymaganych referencji sekretów.

**Zależności:** ARC-01.

### R2-02: Kolejka uploadów MP4

**Pokrywa:** FR-07, NFR-05

**Zakres:** in-memory worker uploadu, maksymalnie trzy próby z backoffem, nazewnictwo plików i metadane żądania.

**Kryteria akceptacji:**

- Ukończony klip jest wysyłany do właściwego bucketa R2 i prefiksu obiektów.
- Awaria sieci nie usuwa pliku lokalnego, a worker wykonuje maksymalnie trzy próby.
- Po wyczerpaniu prób błąd jest logowany; wznowienie po restarcie procesu nie jest wymagane.

**Zależności:** ARC-02, R2-01, VID-03, EVT-01.

### R2-03: Testy klienta i workera Cloudflare R2

**Pokrywa:** FR-07, NFR-03, NFR-05

**Zakres:** testy konfiguracji klienta, poprawnego klucza obiektu, udanego uploadu, limitu trzech prób oraz opcjonalny test z prawdziwym bucketem uruchamiany wyłącznie przy komplecie referencji `CAMWATCH_R2_*`.

**Kryteria akceptacji:**

- Testy jednostkowe workera nie wymagają dostępu do sieci ani bucketa.
- Błąd uploadu nie usuwa lokalnego pliku i kończy się po trzeciej próbie.
- Test opcjonalny z prawdziwym R2 potwierdza zapis i odczyt obiektu testowego.

**Zależności:** R2-01, R2-02.

## Etap 4 — panel WWW

Szczegółowa architektura, decyzje bezpieczeństwa oraz kontrakty między crate'ami są w [specyfikacji backendu SSR](backend-ssr.md).

### WEB-01: Utworzyć crate `camwatch-server` i szkielet HTTP

**Pokrywa:** NFR-01, NFR-04

**Zakres:** dodać `crates/camwatch-server` do workspace'u, binarkę Axum, podstawowy `AppState`, router i kontrolowane uruchomienie serwera wyłącznie na loopbackie. Dodać `askama`, `askama_web`, `tower-http`, `tower-sessions` i pozostałe minimalne zależności serwera. `camwatch-server` importuje `camwatch`; `camwatch` nie może zależeć od HTTP ani HTML.

**Kryteria akceptacji:**

- Workspace buduje oba crate'y.
- Serwer startuje z podstawowym `GET /health`.
- Konfiguracja bindu inna niż `127.0.0.1` lub `::1` jest odrzucana.
- Crate `camwatch` nie zyskuje zależności Axum, Askama, htmx ani sesji.
- Test HTTP potwierdza odpowiedź health bez uruchamiania RTSP, ONVIF ani R2.

**Zależności:** ARC-01.

### WEB-02: Wydzielić `CamwatchService` i publiczny `CamwatchHandle`

**Pokrywa:** NFR-01

**Zakres:** przenieść bootstrap z obecnego `crates/camwatch/src/main.rs` do warstwy aplikacyjnej crate'a `camwatch`. Dodać publiczny `CamwatchService::start` oraz `CamwatchHandle`. Handle ma ukrywać SQLite, GStreamera, ONVIF i kanały workerów, ale pozwolić backendowi na listowanie kamer, odczyt szczegółów, wykonanie PTZ, CRUD kamer i kontrolowane zamknięcie.

**Kryteria akceptacji:**

- `camwatch-server` uruchamia runtime wyłącznie przez publiczny API `camwatch`.
- Publiczny API nie ujawnia `Database`, typów GStreamera ani `OnvifConnection`.
- Błąd konfiguracji R2 przy włączonym R2 nadal kończy start kontrolowanym błędem.
- Zamykanie handle'a kończy workery bez panic.
- Testy publicznego API są w `crates/camwatch/tests/` i nie wymagają HTTP.

**Zależności:** WEB-01, ARC-02, ARC-03, VID-01, R2-02.

### WEB-03: Dodać in-memory rejestr kamer i modele odczytu

**Pokrywa:** FR-01, FR-02, FR-06, FR-10

**Zakres:** dodać rejestr kamer dostępny przez `CamwatchHandle` oraz osobne modele `CameraSummary`, `CameraDetails` i `CameraActivity`. Rejestr powstaje przy starcie, listuje kamery, agreguje status RTSP, wynik wykrycia PTZ i ulotny stan klipu/uploadu. Nie zapisuje historii eventów ani uploadów.

**Kryteria akceptacji:**

- Lista kamer zawiera wszystkie aktywne kamery i nie ujawnia modeli storage.
- Zmiana statusu RTSP jest widoczna przez handle.
- Kamera bez PTZ ma `ptz_available = false`.
- Rejestr udostępnia wyłącznie bieżącą aktywność, bez historii.
- Testy obejmują status online/offline, PTZ oraz stan aktywnego klipu.

**Zależności:** WEB-02, EVT-01, ONV-01.

### WEB-04: Bazowy SSR Askama i full htmx

**Pokrywa:** FR-08, FR-09

**Zakres:** przygotować layout Askama, lokalne assety, strony błędów, renderowanie pełnej strony oraz fragmentów HTML. Dodać htmx lokalnie i skonfigurować `hx-boost`, `hx-push-url` i fallback do zwykłych linków/formularzy. Nie tworzyć JSON API dla panelu.

**Kryteria akceptacji:**

- `GET` renderuje kompletną stronę HTML przez Askama.
- Żądanie z `HX-Request` dostaje wyłącznie wymagany fragment HTML.
- Linki i formularze pozostają użyteczne po wyłączeniu JavaScriptu.
- Są osobne szablony dla layoutu, błędu 403, 404 i 500.
- Każdy model widoku jest osobnym typem w osobnym pliku.

**Zależności:** WEB-01, WEB-03.

### WEB-05: Logowanie i sesja lokalnego administratora

**Pokrywa:** FR-08, NFR-03, NFR-04

**Zakres:** dodać `GET/POST /login`, `POST /logout`, in-memory sesje oraz middleware ochrony tras. Źródłem danych są `CAMWATCH_USER_LOGIN` i `CAMWATCH_USER_PASSWORD`; przy braku obu użyć tylko lokalnego fallbacku `admin/admin`. Sesja wygasa po godzinie bezczynności i znika po restarcie procesu.

**Kryteria akceptacji:**

- Niezalogowany użytkownik jest przekierowany do `/login` dla każdej chronionej trasy.
- Brak obu env pozwala na lokalne logowanie `admin/admin`.
- Ustawienie tylko jednego env lub pustej wartości kończy start kontrolowanym błędem.
- Cookie lokalne ma `HttpOnly`, `SameSite=Strict`, `Path=/` i nie ma `Domain` ani `Secure`.
- Login, hasło, cookie i token CSRF nie trafiają do logów, HTML ani test snapshots.
- Każdy POST jest chroniony przed CSRF.
- Testy obejmują sukces, błąd, logout, wygaśnięcie po godzinie i restart sesji.

**Zależności:** WEB-01, WEB-04.

### WEB-06: Lista kamer i szczegóły kamery

**Pokrywa:** FR-02, FR-06, FR-09

**Zakres:** zaimplementować `GET /cameras`, `GET /cameras/:camera_id` i ich fragmenty htmx. Lista pokazuje karty kamer, status RTSP, dostępność PTZ i bieżącą aktywność. Szczegóły pokazują status, placeholder live preview HLS, PTZ i stan klipu/uploadu. Nie dodawać historii zdarzeń.

**Kryteria akceptacji:**

- Zalogowany użytkownik widzi wszystkie kamery i ich aktualne statusy.
- Szczegóły istniejącej kamery renderują poprawny model widoku.
- Nieistniejąca kamera zwraca SSR 404.
- Fragmenty htmx odświeżają status i aktywność bez przeładowania layoutu.
- Ręczne odświeżenie strony daje ten sam, pełny widok.
- Testy HTTP używają fałszywego handle'a lub kontrolowanego stanu bez prawdziwej kamery.

**Zależności:** WEB-03, WEB-04, WEB-05.

### WEB-07: CRUD kamer z natychmiastowym przeładowaniem runtime'u

**Pokrywa:** FR-01, NFR-01

**Zakres:** rozszerzyć tabelę i modele kamer o `rtsp_codec` oraz `clip_after_motion`, a następnie dodać `CameraInput`, CRUD w SQLite i formularze `/cameras/new`, `/cameras/:camera_id/edit`. Po zapisie utworzyć lub przeładować wyłącznie runtime zmienionej kamery. Usunięcie wykonuje soft-delete i zatrzymuje runtime, bez kasowania segmentów lub klipów.

**Kryteria akceptacji:**

- TOML bootstrapuje tylko pustą bazę; po nim SQLite jest źródłem prawdy dla kamer.
- Formularze przyjmują wyłącznie referencje env dla RTSP i ONVIF, nigdy sekrety.
- Nieprawidłowe dane renderują błędy walidacji jako pełna strona lub fragment htmx.
- Dodanie, edycja i usunięcie zmieniają SQLite oraz rejestr kamer.
- Edycja jednej kamery nie zatrzymuje pozostałych runtime'ów.
- Nieudany start nowego runtime'u daje kontrolowany status i nie powoduje panic.
- Testy obejmują CRUD, soft-delete i przeładowanie pojedynczej kamery.

**Zależności:** WEB-02, WEB-03, WEB-05, WEB-06.

### WEB-08: Panel PTZ jako krótkie ruchy htmx

**Pokrywa:** FR-10

**Zakres:** dodać cztery przyciski kierunkowe do szczegółów kamery. Każdy klik wykonuje jeden bezpieczny, krótki ruch przez `CamwatchHandle::move_ptz`; nie implementować przytrzymywania przycisku. Odpowiedź htmx odświeża komponent PTZ i komunikat błędu.

**Kryteria akceptacji:**

- Przycisk wysyła pojedynczą komendę góra/dół/lewo/prawo.
- Kamera bez PTZ nie pokazuje kontrolek, a endpoint odmawia komendy.
- Równoległe komendy dla tej samej kamery są serializowane przez core.
- Błąd ONVIF nie ujawnia szczegółów implementacji i nie psuje widoku kamery.
- Testy pokrywają sukces, brak PTZ i błąd sterowania.

**Zależności:** WEB-06, ONV-01.

### VID-04: Pipeline HLS dla podglądu WWW

**Pokrywa:** FR-09

**Zakres:** dodać pipeline HLS per kamera, katalog playlist i retencję fragmentów. Pipeline jest własnością crate'a `camwatch`; nie wystawia bezpośrednio katalogu `data/` przez HTTP.

**Kryteria akceptacji:**

- Aktywna kamera tworzy odtwarzalną playlistę HLS.
- Niedostępna kamera nie blokuje playlist innych kamer.
- HLS może być odczytane przez bezpieczny identyfikator kamery, bez ścieżki podanej przez użytkownika.
- Testy pipeline'u nie wymagają przeglądarki.

**Zależności:** VID-01, WEB-02.

### WEB-09: Chronione HLS i odtwarzacz `hls.js`

**Pokrywa:** FR-09, NFR-03

**Zakres:** dodać uwierzytelnione endpointy playlist i segmentów HLS oraz lokalnie serwowany `hls.js`. Widok kamery używa natywnego HLS w Safari, a `hls.js` w Chrome i Firefox. Nie używać CDN i nie wystawiać `data/` jako publicznego katalogu.

**Kryteria akceptacji:**

- Niezalogowany użytkownik nie odczyta playlisty ani segmentu HLS.
- Endpoint przyjmuje wyłącznie znane `camera_id`; nie pozwala na path traversal.
- Safari dostaje natywny `video` HLS, Chrome i Firefox używają lokalnego `hls.js`.
- Błąd playlisty jest widoczny w komponencie kamery i nie ujawnia ścieżek systemowych.
- Testy HTTP pokrywają autoryzację, nieistniejącą kamerę i blokadę path traversal.

**Zależności:** VID-04, WEB-05, WEB-06.

### WEB-10: Testy odbiorcze i hardening panelu

**Pokrywa:** FR-01, FR-08, FR-09, FR-10, NFR-03, NFR-04, NFR-06

**Zakres:** domknąć testy publicznego zachowania panelu, request tracing, bezpieczne nagłówki i kontrolę błędów. Zweryfikować, że wszystkie dynamiczne ścieżki htmx mają pełny SSR fallback.

**Kryteria akceptacji:**

- Testy end-to-end panelu pokrywają login → lista → szczegóły → edycja kamery → PTZ → logout.
- Każda trasa htmx działa także jako zwykły request HTML.
- Logi nie zawierają sekretów, tokenów, cookies, RTSP ani credentiali R2.
- Nieautoryzowany HLS i PTZ są odrzucone.
- `cargo test --workspace`, Clippy i `git diff --check` przechodzą.

**Zależności:** WEB-07, WEB-08, WEB-09.

## Etap 5 — niezawodność i wydanie

### OPS-01: Obserwowalność i odzyskiwanie błędów

**Pokrywa:** NFR-01, NFR-05, NFR-06

**Zakres:** logi strukturalne, health check, metryki aktywnych klipów/prób uploadu/reconnectów i limity dysku.

**Kryteria akceptacji:**

- Health check odróżnia działającą aplikację od stanu z niedostępną pojedynczą kamerą.
- Przekroczenie limitu dysku nie usuwa klipów oczekujących na upload.
- Log pozwala ustalić przyczynę nieudanego uploadu i reconnectu.

**Zależności:** VID-01, VID-03, R2-02.

### TST-01: Testy integracyjne oraz test odbiorczy MVP

**Pokrywa:** wszystkie FR i NFR

**Zakres:** fałszywy RTSP/ONVIF/R2, scenariusze e2e i ręczna checklista z prawdziwą kamerą Tapo. Testy R2 są zaplanowane jako najbliższy krok.

**Kryteria akceptacji:**

- Test e2e potwierdza przepływ: ruch → osoba → MP4 → in-memory worker → upload.
- Test awarii RTSP potwierdza reconnect bez wyłączenia serwera WWW.
- Test autoryzacji potwierdza brak dostępu do HLS i klipów bez sesji.
- Ręczny test PTZ i podglądu na prawdziwej kamerze kończy się pozytywnie.

**Zależności:** wszystkie wcześniejsze taski.

## Kolejność pierwszego wdrożenia

`ARC-01 → ARC-02 → ARC-03 → VID-01 → VID-02 → DET-01 → DET-02 → VID-03 → EVT-01 → R2-01 → R2-02 → R2-03 → WEB-01 → WEB-02 → WEB-03 → WEB-04 → WEB-05 → WEB-06 → WEB-07 → WEB-08 → VID-04 → WEB-09 → WEB-10 → OPS-01 → TST-01`
