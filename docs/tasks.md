# Backlog spec-driven

Każdy task jest powiązany z wymaganiami z [dokumentu przewodniego](README.md). Task można uznać za ukończony dopiero po spełnieniu wszystkich kryteriów akceptacji oraz dodaniu testów właściwych dla jego warstwy.

## Zasady realizacji

- Implementujemy najpierw pionowy przepływ dla jednej kamery, a dopiero potem skalowanie do wielu kamer.
- Wszystkie operacje I/O są asynchroniczne lub wykonywane na wydzielonych workerach.
- Żaden sekret nie trafia do kodu, konfiguracji wersjonowanej ani logów.
- Każdy moduł ma jawne błędy domenowe i logi strukturalne.
- Elementy RTSP, ONVIF i Google Drive otrzymują adaptery, aby testy nie wymagały prawdziwej kamery ani konta Google.

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

**Pokrywa:** FR-06, NFR-05

**Zakres:** migracje `cameras`, `events`, `uploads`, repozytoria oraz modele statusów.

**Kryteria akceptacji:**

- Migracje są idempotentne.
- Zdarzenie i rekord uploadu można zapisać oraz odczytać.
- Restart aplikacji nie usuwa danych.

**Zależności:** ARC-01.

### ARC-03: Zdefiniować porty i adaptery infrastruktury

**Pokrywa:** wszystkie FR

**Zakres:** interfejsy `CameraStream`, `MotionDetector`, `PersonDetector`, `ClipStore`, `DriveUploader`, `PtzController` i ich implementacje testowe.

**Kryteria akceptacji:**

- Silnik zdarzeń można uruchomić z fałszywą kamerą i fałszywym uploaderem.
- Kod domenowy nie importuje bezpośrednio GStreamera, ONVIF ani HTTP Google.

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

### VID-04: HLS dla podglądu WWW

**Pokrywa:** FR-09

**Zakres:** pipeline HLS per kamera, czyszczenie starych segmentów i endpoint do serwowania playlisty.

**Kryteria akceptacji:**

- Aktualny obraz kamery można otworzyć w przeglądarce przez HLS.
- Opóźnienie jest mierzalne i prezentowane w statusie kamery.
- Niedostępna kamera nie blokuje playlist innych kamer.

**Zależności:** VID-01.

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

**Zakres:** stany `idle`, `candidate`, `recording`, `finalizing`, `upload_pending`, `uploaded`, `upload_failed`; cooldown i rozszerzanie zdarzeń.

**Kryteria akceptacji:**

- Ruch tworzy zdarzenie, a YOLO wzbogaca je o metadane osoby.
- Seria ruchów w oknie cooldown nie tworzy duplikatów.
- Zdarzenie przechodzi do finalizacji dopiero po ciszy wymaganej dla post-bufferu.

**Zależności:** ARC-02, VID-03, DET-01, DET-02.

## Etap 3 — integracje zewnętrzne

### ONV-01: Odczyt możliwości ONVIF i PTZ

**Pokrywa:** FR-01, FR-10

**Zakres:** połączenie ONVIF, profil media, sprawdzenie PTZ i komendy `ContinuousMove` oraz `Stop`.

**Kryteria akceptacji:**

- Aplikacja zapisuje, czy kamera zgłasza PTZ.
- Ruch we wszystkich czterech kierunkach oraz zatrzymanie działa na obsługiwanej kamerze.
- Kamera bez PTZ nie powoduje błędu krytycznego.

**Zależności:** ARC-03.

### GDR-01: Konfiguracja Google OAuth i folderu docelowego

**Pokrywa:** FR-07, NFR-03

**Zakres:** lokalny flow OAuth, bezpieczne zapisanie tokenu odświeżania i wybór folderu Drive.

**Kryteria akceptacji:**

- Użytkownik przechodzi autoryzację w przeglądarce tylko podczas konfiguracji.
- Token nie jest zapisywany w repozytorium, logach ani jawnej konfiguracji.
- Aplikacja potrafi zweryfikować dostęp do wybranego folderu.

**Zależności:** ARC-01.

### GDR-02: Kolejka uploadów MP4

**Pokrywa:** FR-07, NFR-05

**Zakres:** worker uploadu, status w SQLite, retry z backoffem, nazewnictwo plików i metadane zdarzenia.

**Kryteria akceptacji:**

- Ukończony klip jest wysyłany do właściwego folderu Drive.
- Awaria sieci oznacza `upload_failed` lub `upload_pending`, bez usunięcia pliku lokalnego.
- Po przywróceniu sieci upload zostaje wznowiony bez interwencji użytkownika.

**Zależności:** ARC-02, GDR-01, VID-03, EVT-01.

## Etap 4 — panel WWW

### WEB-01: Uwierzytelnianie i sesja administratora

**Pokrywa:** FR-08, NFR-03, NFR-04

**Zakres:** inicjalne ustawienie hasła, Argon2id, sesja HTTP-only, logout i ochrona wszystkich endpointów.

**Kryteria akceptacji:**

- Niezalogowany użytkownik nie odczyta API, HLS ani nagrań.
- Hasło nigdy nie występuje w logach ani bazie w formie jawnej.
- Aplikacja domyślnie nasłuchuje tylko lokalnie.

**Zależności:** ARC-01.

### WEB-02: Widok kamer i historia zdarzeń

**Pokrywa:** FR-06, FR-09

**Zakres:** lista statusów kamer, odtwarzacz HLS, historia zdarzeń, szczegóły oraz lokalny klip MP4.

**Kryteria akceptacji:**

- Użytkownik widzi status każdej kamery i jej aktualny podgląd.
- Użytkownik może odtworzyć klip zdarzenia.
- Widok pokazuje typ zdarzenia, liczbę osób i status Drive.

**Zależności:** ARC-02, VID-04, EVT-01, WEB-01.

### WEB-03: Sterowanie PTZ w panelu

**Pokrywa:** FR-10

**Zakres:** przyciski kierunkowe, ruch podczas przytrzymania, natychmiastowe zatrzymanie i informacja o braku PTZ.

**Kryteria akceptacji:**

- Przytrzymanie przycisku wysyła ruch, a puszczenie wysyła `Stop`.
- Użytkownik widzi błąd sterowania bez utraty sesji i podglądu.
- Elementy PTZ są ukryte lub nieaktywne dla kamery bez PTZ.

**Zależności:** ONV-01, WEB-01, WEB-02.

## Etap 5 — niezawodność i wydanie

### OPS-01: Obserwowalność i odzyskiwanie błędów

**Pokrywa:** NFR-01, NFR-05, NFR-06

**Zakres:** logi strukturalne, health check, metryki liczby zdarzeń/uploadów/reconnectów i limity dysku.

**Kryteria akceptacji:**

- Health check odróżnia działającą aplikację od stanu z niedostępną pojedynczą kamerą.
- Przekroczenie limitu dysku nie usuwa klipów oczekujących na upload.
- Log pozwala ustalić przyczynę nieudanego uploadu i reconnectu.

**Zależności:** VID-01, VID-03, GDR-02.

### TST-01: Testy integracyjne oraz test odbiorczy MVP

**Pokrywa:** wszystkie FR i NFR

**Zakres:** fałszywy RTSP/ONVIF/Drive, scenariusze e2e i ręczna checklista z prawdziwą kamerą Tapo.

**Kryteria akceptacji:**

- Test e2e potwierdza przepływ: ruch → osoba → MP4 → rekord → upload.
- Test awarii RTSP potwierdza reconnect bez wyłączenia serwera WWW.
- Test autoryzacji potwierdza brak dostępu do HLS i klipów bez sesji.
- Ręczny test PTZ i podglądu na prawdziwej kamerze kończy się pozytywnie.

**Zależności:** wszystkie wcześniejsze taski.

## Kolejność pierwszego wdrożenia

`ARC-01 → ARC-02 → ARC-03 → VID-01 → VID-02 → DET-01 → DET-02 → VID-03 → EVT-01 → GDR-01 → GDR-02 → WEB-01 → VID-04 → WEB-02 → ONV-01 → WEB-03 → OPS-01 → TST-01`
