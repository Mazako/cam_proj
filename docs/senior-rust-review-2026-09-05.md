# Audyt techniczny Camwatch jako aplikacji Rust

Data przeglądu: 2026-09-05

Zakres: bieżący working tree repozytorium `/Users/michalmaziarz/repos/cam_proj`, commit bazowy `8a3b92d` (`PTZ`) oraz niezacommitowana zmiana `README.md` obecna podczas audytu.

## Werdykt

Camwatch ma sensowny szkielet i kilka dojrzałych decyzji: osobny crate HTTP, typowaną konfigurację, SQLite z migracjami, jawne adaptery infrastruktury, szyfrowanie sekretów, kontrolę dostępu do HLS i wartościowe testy z prawdziwym GStreamerem. To nie jest chaotyczny prototyp.

Jednocześnie kod nie spełnia jeszcze rygorystycznego kryterium „bardzo clean, idiomatyczny i niezawodny”. Największe problemy nie są kosmetyczne: istnieje wyścig mogący usunąć segment potrzebny aktywnemu klipowi, konfiguracja `motion_min_area` nie wpływa na runtime, kolejne detekcje nie wydłużają aktywnego zdarzenia, a pipeline tła nie ma kontrolowanego backpressure, pełnego shutdownu ani timeoutu składania klipu. Produkcyjna ścieżka analizy obrazu zawiera też `unwrap()` i wykonuje kosztowne obliczenia bezpośrednio w tasku Tokio.

Ocena surowa:

| Obszar | Ocena | Uzasadnienie |
| --- | ---: | --- |
| Architektura modułów | 6/10 | Dobry podział crate'ów, ale część deklarowanych portów jest martwa, a `AppState` jest szerokim orchestrator/service locator. |
| Idiomatyczność Rust | 6/10 | Dobre typy i `thiserror`, lecz produkcyjne `unwrap()`, ręczne niespójne porty, utrata błędów i niejawny lifecycle tasków obniżają ocenę. |
| Poprawność domenowa | 4/10 | Próg ruchu jest ignorowany, zdarzenia nie są rozszerzane, a retencja ma TOCTOU. |
| Niezawodność i współbieżność | 3/10 | Nieograniczone kolejki, brak nadzoru workerów, blokujący backoff i brak timeoutu assembly. |
| Bezpieczeństwo lokalnego MVP | 6/10 | Dobre szyfrowanie, CSRF i loopback; brakuje rate limitu, a typy z sekretami implementują `Debug`. |
| Testy | 6/10 | Mocne testy behawioralne i RTSP, ale brak krytycznych scenariuszy współbieżności; pełny suite wykazał flake HLS. |
| Obserwowalność i operacje | 3/10 | `health` jest wyłącznie liveness, błędy R2 są spłaszczane, task panic jest ignorowany. |
| Gotowość produkcyjna | 3/10 | Dobra baza do dalszej pracy, ale jeszcze nie system, któremu można powierzyć niezawodne zachowanie materiału dowodowego. |

## Najważniejsze ustalenia

Priorytety:

- **P0** — możliwa utrata klipu lub złamanie podstawowej gwarancji systemu w normalnej pracy;
- **P1** — poważny błąd poprawności, dostępności albo kontroli zasobów;
- **P2** — istotny dług techniczny, luka testowa lub niespójność kontraktu;
- **P3** — porządek, ergonomia i dalsze uproszczenia.

### P0 — retencja może usunąć segment już wybrany do aktywnego klipu

`retain_expired_segments()` najpierw osobno pyta `is_segment_reserved()`, a następnie asynchronicznie usuwa plik. Rezerwacja w `SegmentLease::reserve()` odbywa się niezależnie. Sprawdzenie i usunięcie nie tworzą jednej operacji atomowej.

Dowody:

- `crates/camwatch/src/clips/segment_retainer.rs:45-63`;
- `crates/camwatch/src/clips/clip_manager.rs:54-65`;
- `crates/camwatch/src/clips/segment_lease.rs:20-29`.

Możliwe przeplecenie:

1. Retainer odczytuje stary segment z SQLite.
2. Retainer widzi, że ścieżka nie jest jeszcze zarezerwowana.
3. `add_clip()` pobiera ten sam segment i dodaje go do lease aktywnego klipu.
4. Retainer usuwa plik.
5. Assembly dostaje ścieżkę nieistniejącego pliku i cały klip przepada.

Obecne testy wykonują retencję i rezerwację sekwencyjnie, więc nie mogą wykryć tego wyścigu.

Rekomendacja: wprowadzić jeden właścicielski automat stanu segmentu, np. `Available -> Reserved(n)` albo `Available -> Deleting`. Claim do usunięcia musi być synchroniczną, atomową operacją względem lease. Nie należy trzymać guarda `DashMap` przez `.await`; po skutecznym przejściu do `Deleting` można zwolnić guard i dopiero wykonać I/O. `reserve()` musi odrzucać segment oznaczony jako usuwany.

### P1 — `motion_min_area` jest konfiguracją pozorną

Próg jest walidowany, zapisywany w SQLite i edytowalny w panelu, ale MOG2 używa stałej `MIN_MOTION_AREA = 1_000.0`. `CameraRuntime` tworzy detektor przez `Mog2MotionDetector::new()` bez wartości z `CameraConfig`.

Dowody:

- `crates/camwatch/src/config/camera.rs:15`;
- `crates/camwatch/src/motion/mog2.rs:10-12` i `51-57`;
- `crates/camwatch/src/runtime/camera_runtime.rs:43`.

Skutek: użytkownik może zapisać np. `2000`, UI i baza pokażą `2000`, ale zachowanie pozostanie takie jak dla `1000`. To jest błąd funkcjonalny, nie preferencja stylistyczna. Niezacommitowany `README.md` dodatkowo twierdzi, że to pole filtruje kontur, czego runtime obecnie nie realizuje.

Rekomendacja: konstruktor `Mog2MotionDetector::new(min_motion_area)` z typem gwarantującym wartość dodatnią. Test runtime powinien użyć dwóch różnych progów na identycznej sekwencji klatek i wykazać różne decyzje.

### P1 — kolejne detekcje nie wydłużają aktywnego zdarzenia

`handle_frame_event()` natychmiast wraca, gdy kamera ma aktywny klip. `ActiveClip` nie ma operacji rozszerzenia `ended_at`; koniec jest wyliczany raz od pierwszego triggera.

Dowody:

- `crates/camwatch/src/runtime/camera_runtime.rs:130-136`;
- `crates/camwatch/src/clips/active_clip.rs:20-39`;
- `docs/README.md:73-75`;
- `docs/tasks.md`, EVT-01: seria ruchów w cooldownie ma nie tworzyć duplikatów i finalizacja ma nastąpić po wymaganej ciszy.

Skutek: ruch trwający dłużej niż `post_event_seconds` zostanie obcięty. Po finalizacji kolejna klatka może rozpocząć drugi klip, więc implementacja nie odpowiada ani „wydłuż jeden klip”, ani „finalizuj po ciszy”. Test nazwany w nowym `README.md` jako pokrycie overlapping event clips w praktyce sprawdza współdzielone lease dwóch ręcznie utworzonych jobów, nie rozszerzanie zdarzenia przez runtime.

Rekomendacja: `ClipManager::trigger(camera_id, detected_at, ...)` powinien atomowo utworzyć klip albo przesunąć `ended_at` do `max(current, detected_at + post)`. Runtime nie powinien pomijać detekcji tylko dlatego, że trwa nagrywanie.

### P1 — kosztowna analiza obrazu blokuje executor Tokio

MOG2 i inferencja ONNX są wywoływane synchronicznie bezpośrednio wewnątrz `CameraRuntime::run()`, uruchomionego przez `tokio::spawn`. YOLO ma osobną sesję per kamera, mimo że dokumentacja zakłada pulę workerów.

Dowody:

- `crates/camwatch-server/src/runtime_task.rs:14-22`;
- `crates/camwatch/src/runtime/camera_runtime.rs:82-95` i `182-192`;
- `crates/camwatch/src/motion/yolo_analyzer.rs:27-51`;
- `docs/tasks.md`, DET-02: inferencja nie może blokować RTSP i ma korzystać z puli workerów.

Skutek: wolna inferencja zajmuje wątek executora, opóźnia HTTP i inne kamery, a runtime przestaje konsumować eventy segmentów. Bufor streamu ma tylko osiem elementów; klatki są wprawdzie zrzucane, ale event segmentu używa `blocking_send`, więc zator może przejść z analizy do workera GStreamera.

Rekomendacja: wydzielić ograniczoną pulę CPU/inference, maksymalnie jeden oczekujący frame per kamera i jawny drop policy. Wynik powinien wracać jako zdarzenie z korelacją czasu. Nie tworzyć bez potrzeby pełnej sesji ONNX dla każdej kamery.

### P1 — produkcyjna ścieżka może panicować na błędzie infrastruktury lub danych

`CameraRuntime::new()` robi `unwrap()` na inicjalizacji OpenCV i ONNX, a analiza frame robi `unwrap()` na obu detektorach. `ActiveClip` i nazwa pliku też zakładają bezbłędną arytmetykę `SystemTime`.

Dowody:

- `crates/camwatch/src/runtime/camera_runtime.rs:43-48`, `183` i `191`;
- `crates/camwatch/src/clips/active_clip.rs:29-30`;
- `crates/camwatch/src/clips/clip_manager.rs:105-107`.

To jest sprzeczne z wymaganiem, aby awaria jednej kamery nie zatrzymywała innych części systemu. Panic w runtime nie kończy całego procesu przy domyślnym panic unwind, ale task umiera po cichu, ponieważ `RuntimeTask::stop()` ignoruje `JoinError`.

Rekomendacja: konstruktor ma zwracać typowany `Result`; błąd pojedynczej klatki powinien mieć kontrolowaną politykę (metryka/log, pominięcie frame, ewentualnie degradacja runtime), a task supervisor ma rejestrować panic i niespodziewane zakończenie.

### P1 — pipeline klipów nie ma backpressure, limitu czasu ani zarządzanego shutdownu

Oba workery używają `mpsc::unbounded_channel()`. Assembly czeka na GStreamer przez `timed_pop(ClockTime::NONE)`, czyli bez limitu. Handlery workerów są odrzucane po `tokio::spawn`, a główny shutdown zatrzymuje tylko runtime kamer.

Dowody:

- `crates/camwatch/src/clips/clip_saver.rs:7-19`;
- `crates/camwatch/src/clips/clip_uploader.rs:10-19`;
- `crates/camwatch/src/clips/clip_store.rs:137-151`;
- `crates/camwatch/src/clips/segment_retainer.rs:13-24`;
- `crates/camwatch-server/src/main.rs:58-67`.

Jeden zawieszony assembly blokuje jedynego consumera na zawsze. Kolejne joby rosną bez limitu, a ich lease blokują retencję segmentów, więc awaria może zamienić się w nieograniczony wzrost dysku i pamięci. Przy shutdownie in-flight assembly lub upload może zostać przerwany bez raportu i bez drain.

Rekomendacja: bounded channels, jawna polityka przeciążenia, timeout per assembly/upload, `CancellationToken`, przechowywane `JoinHandle`, zamknięcie senderów, drain z deadline oraz wynik shutdownu zwracany do `main`.

### P1 — reload kamery nie zachowuje ostatniej działającej konfiguracji

Edycja najpierw utrwala nową konfigurację. `replace_camera_runtime()` następnie zatrzymuje stary runtime, dopiero potem próbuje utworzyć nowy stream. Błąd startu jest tylko logowany, a handler i tak zwraca redirect sukcesu.

Dowody:

- `crates/camwatch-server/src/camera_routes.rs:203-208` i `323-344`;
- `crates/camwatch-server/src/app_state.rs:164-193`.

Skutek: literówka albo przejściowy błąd inicjalizacji może wyłączyć wcześniej działającą kamerę, a użytkownik nie dostaje sygnału, że zastosowanie konfiguracji nie powiodło się. Trwały rekord i aktywny runtime przestają tworzyć spójny stan.

Rekomendacja: przygotować nowy runtime przed odstawieniem starego, a następnie wykonać kontrolowany swap. Jeśli pełne przygotowanie wymaga rozpoczęcia streamu, potrzebny jest jawny stan `Starting/Running/Failed` i odpowiedź UI pokazująca wynik. Należy ustalić, czy trwała konfiguracja może być `desired state`, czy baza ma opisywać wyłącznie stan zastosowany.

### P1 — błędy R2 tracą przyczynę dokładnie tam, gdzie jest potrzebna diagnostyka

`R2Client` buduje bogaty `R2Error`, ale implementacja portu zamienia każdy błąd na `BucketUploaderError::Failed`. Worker loguje już tylko komunikat generyczny.

Dowody:

- `crates/camwatch/src/bucket/client.rs:44-59` i `73-82`;
- `crates/camwatch/src/bucket/error.rs:3-17`;
- `crates/camwatch/src/bucket/uploader_error.rs:3-9`;
- `crates/camwatch/src/clips/clip_uploader.rs:24-35`.

Nie da się odróżnić błędu odczytu lokalnego pliku, DNS/TLS, autoryzacji, bucketa i odpowiedzi usługi. Jest to sprzeczne z NFR-06 oraz kryterium OPS-01 mówiącym, że log ma wyjaśniać przyczynę uploadu.

Rekomendacja: zachować bezpieczny łańcuch przyczyn i klasyfikację retryable/non-retryable. Nie logować credentiali ani pełnych nagłówków; można logować klasę błędu, status HTTP, request ID i nazwę operacji.

### P1 — zatrzymanie workera RTSP może czekać cały backoff

Między próbami reconnect worker wykonuje `thread::sleep(delay)`. Token anulowania jest sprawdzany dopiero po obudzeniu, a `shutdown()` czeka na `join()` tego wątku. Opóźnienie dochodzi do dziesięciu sekund, zaś `stop_all_runtimes()` zatrzymuje kamery sekwencyjnie.

Dowody:

- `crates/camwatch/src/stream/gstreamer.rs:79-88`;
- `crates/camwatch/src/stream/gstreamer_camera_stream.rs:63-71`;
- `crates/camwatch-server/src/app_state.rs:152-161`.

Dla czterech kamer shutdown/reload może w najgorszym przepleceniu trwać około 40 sekund. Test graceful shutdown używa fałszywego streamu i nie obejmuje backoffu prawdziwego adaptera.

Rekomendacja: przerwalne oczekiwanie na token/condvar oraz równoległe zatrzymywanie niezależnych runtime'ów z globalnym deadline.

### P2 — publiczne typy i porty nie opisują faktycznej architektury

`PersonDetector`, `PersonDetection`, `BoundingBox` i `PersonDetectorError` są eksportowane, ale produkcyjny `YoloAnalyzer` nie implementuje tego portu. Runtime zależy bezpośrednio od `Mog2MotionDetector` i `YoloAnalyzer`. `PersonDetector` oczekuje `&self + Send + Sync`, natomiast używany analyzer wymaga `&mut self`.

Dowody:

- `crates/camwatch/src/motion/mod.rs:15-26`;
- `crates/camwatch/src/motion/person_detector.rs:7-13`;
- `crates/camwatch/src/runtime/camera_runtime.rs:18-28`;
- `crates/camwatch/tests/interfaces_test.rs:7-14`.

Test sprawdza wyłącznie object safety martwego interfejsu. To nie jest użyteczna granica architektoniczna.

Rekomendacja: albo naprawdę wstrzyknąć porty do runtime'u i testować przez fake'i, albo usunąć abstrakcje do czasu pojawienia się drugiej implementacji. W tym projekcie rozsądny jest generyczny lub trait-object `DetectorBundle`, ale tylko jeśli umożliwi testowanie polityki runtime bez OpenCV/ONNX.

### P2 — błędy inferencji mogą wyglądać jak poprawny brak detekcji

Gdy output ONNX nie daje się odczytać jako tensor `f32`, kod pisze przez `eprintln!` i kontynuuje, zwracając potencjalnie pustą listę. Dodatkowo `DetectionClass::from_class_id(...).unwrap()` opiera bezpieczeństwo na sąsiednim warunku.

Dowód: `crates/camwatch/src/motion/yolo_analyzer.rs:34-50`.

Rekomendacja: niepoprawny kontrakt modelu ma zwracać `YoloAnalyzerError`, a filtrowanie klas można zapisać przez `filter_map`, bez częściowego dopasowania plus `unwrap()`. Logowanie powinno iść przez `tracing` na poziomie runtime'u.

### P2 — sekrety są chronione w storage, lecz zbyt łatwe do przypadkowego zalogowania

`R2Config` ma poprawnie redagowany `Debug`, lecz `Config`, `AppConfig`, `CameraConfig`, `CameraDetailsDto` i `CameraInput` implementują lub składają `Debug` nad polami zawierającymi odszyfrowane URL-e i credentiale. Aktualnie nie znalazłem miejsca, które je loguje, ale bariera bezpieczeństwa opiera się na dyscyplinie każdego przyszłego wywołania.

Dowody:

- `crates/camwatch/src/config/settings.rs:7-13`;
- `crates/camwatch/src/config/app.rs:8-37`;
- `crates/camwatch/src/config/camera.rs:7-19`;
- `crates/camwatch-server/src/camera_dto/camera_details_dto.rs:3-12`;
- `crates/camwatch-server/src/camera_dto/camera_input.rs:7-21`.

Rekomendacja: wrapper `SecretString` z redagowanym `Debug`, kontrolowanym dostępem do plaintextu i opcjonalnym `zeroize`. DTO szczegółów dla zwykłego widoku nie powinien w ogóle przenosić sekretów; formularz edycji już poprawnie renderuje pola sekretne jako puste.

### P2 — zdrowie procesu i status runtime'u są zbyt płytkie

`GET /health` zawsze zwraca `200 ok`, bez sprawdzenia SQLite, workerów i kamer. `RuntimeTask::is_running()` mówi tylko, czy task jest zakończony; wynik taska jest ignorowany. Status streamu może pozostać `Online` po niespodziewanym zamknięciu abstrakcyjnego `CameraStream`, ponieważ handler błędu nie aktualizuje modelu.

Dowody:

- `crates/camwatch-server/src/router.rs:82-84`;
- `crates/camwatch-server/src/runtime_task.rs:35-42`;
- `crates/camwatch/src/runtime/camera_runtime.rs:97-103`;
- `docs/tasks.md:403-415`.

Liveness `200` jest przydatne, ale powinno być nazwane i oddzielone od readiness/degraded status. OPS-01 jest nadal realnie niezrealizowany.

### P2 — wymagania i dokumentacja dryfują

Przykłady:

- `docs/README.md:141` mówi o Argon2id, podczas gdy świadoma decyzja w `docs/backend-ssr.md:396-408` i kod stosują plaintext z env oraz porównanie constant-time;
- dokument przewodni opisuje potwierdzanie osoby, a `YoloAnalyzer` akceptuje także kota i psa;
- backlog wymaga limitu wieku i rozmiaru segmentów, a konfiguracja oraz retainer implementują tylko wiek;
- niezacommitowany `README.md` deklaruje użycie `motion_min_area` i pokrycie overlapping event clips, których bieżący runtime nie realizuje.

Dokumentacja jest częścią kontraktu systemu. Taki dryf utrudnia testowanie i może prowadzić do błędnych decyzji operacyjnych.

### P2 — test HLS jest niestabilny i łamie izolację warstwy HTTP

Podczas audytu pełne `cargo test --workspace -- --test-threads=1` przeszło wszystkie wcześniejsze testy, ale `camwatch-server/tests/hls_test.rs` raz dostał `404` zamiast `200`. Ten sam test uruchomiony osobno przeszedł, podobnie jak późniejszy pełny pakiet `camwatch-server`.

Test bootstrappuje prawdziwy `GstreamerCameraStream` dla nieosiągalnego hosta, a endpoint HLS wymaga `RuntimeTask::is_running()`. Tym samym test serwowania plików zależy od lifecycle wątku GStreamera i schedulera, mimo że specyfikacja backendu mówi, że testy HTTP nie powinny wymagać GStreamera.

Dowody:

- `crates/camwatch-server/tests/hls_test.rs:14-50`;
- `crates/camwatch-server/tests/support/mod.rs:16-60`;
- `crates/camwatch-server/src/hls_routes.rs:59-67`;
- `docs/backend-ssr.md:510`.

Rekomendacja: HLS handler powinien zależeć od małego interfejsu/read modelu, a test wstawić deterministyczny runtime status bez uruchamiania natywnej infrastruktury. Osobny test integracyjny powinien sprawdzać realny pipeline HLS.

### P3 — mniejsze problemy jakościowe

- `camera_routes.rs` ma 399 linii i skupia CRUD, PTZ, CSRF, mapowanie błędów i orchestration reloadu. Podział na `camera_crud`, `camera_query` i `ptz_routes` poprawiłby lokalność zmian.
- `AppState` wystawia większość pól publicznie i łączy stan HTTP, bazę, runtime registry, sekrety oraz clip manager. Warto ograniczyć pola i udostępnić metody o znaczeniu domenowym.
- `runtime_reload_locks` i `CameraStatusModel` nie usuwają wpisów po trwałym usunięciu kamery. Przy niewielkiej liczbie kamer wpływ jest mały, ale lifecycle rejestrów jest niepełny.
- `CameraId`, nazwa kamery i credentiale ONVIF nie mają rozsądnych limitów długości. `onvif_credentials` jest luźnym `String` parsowanym dopiero w adapterze jako `username:password`.
- Repozytorium wersjonuje około 210 MB fixture'ów PETS2006 w 2406 plikach oraz model ONNX około 9,5 MB. Fixture jest wartościowy, ale koszt checkoutu i CI powinien być świadomie zaakceptowany albo ograniczony do minimalnego reprezentatywnego wycinka.

## Co jest zrobione dobrze

Surowy audyt nie oznacza ignorowania mocnych stron:

- Podział `camwatch` / `camwatch-server` / `camwatch-secret` jest trafny. Core nie importuje Axum, Askama ani sesji.
- `#[serde(deny_unknown_fields)]`, walidacja URL-i, `CameraId` i agregacja błędów konfiguracji są sensownie zaprojektowane.
- Wyłączenie R2 rzeczywiście short-circuituje odczyt jego sekretów.
- AES-256-GCM używa losowego nonce, a plaintext sekretów nie jest wersjonowany w konfiguracji przykładowej.
- SQLite ma WAL, foreign keys, constraints i transakcję przy zbiorczym upsercie kamer.
- `SegmentLease` z licznikiem poprawnie rozwiązuje przypadek współdzielenia segmentu przez więcej niż jeden job; problem leży w atomowości z retainerem, nie w samym liczniku.
- Składanie MP4 jest poprawnie przeniesione do `spawn_blocking`.
- Adapter RTSP działa na dedykowanym wątku, ma ograniczony bufor frame'ów i reconnect z jitterem.
- Panel używa Askamy, sesji server-side, rotacji session ID, `HttpOnly`, `SameSite=Strict` i tokenów CSRF dla mutacji.
- HLS nie wystawia całego katalogu danych, a camera ID i nazwa segmentu są walidowane przed mapowaniem do ścieżki.
- Edycja nie renderuje aktualnych sekretów w HTML; puste pole zachowuje dotychczasową wartość.
- Testy obejmują realny GStreamer/RTSP przez Docker, reconnect, składanie MP4, MOG2 na PETS2006, YOLO, storage, sesje, CRUD i mock ONVIF. To jest znacznie lepszy punkt wyjścia niż testy wyłącznie konstruktorów.
- CI wymusza formatowanie, testy, Clippy z `-D warnings` i `git diff --check`, a test prawdziwego R2 jest oddzielony od zwykłego CI.

## Ocena API i idiomatyczności

### Typy i błędy

`thiserror`, osobne błędy konfiguracji/storage i newtype `CameraId` są idiomatyczne. Problemem jest nierówny poziom szczegółowości: błędy configu są dobre, ale stream redukuje wszystko do `Unavailable/Failed`, motion do `Failed`, a bucket ponownie redukuje bogaty SDK error do `Failed`. Callery nie mogą podjąć rozsądnej decyzji o retry, degradacji ani komunikacie.

Docelowo błędy powinny być szczegółowe na granicy adaptera, ale bezpieczne na granicy UI/logu. Nie trzeba wystawiać całego `SdkError`; trzeba zachować klasyfikację i `source()`.

### Ownership i współbieżność

Użycie `Arc`, `DashMap` i `CancellationToken` jest generalnie uzasadnione, ale nie tworzy jeszcze poprawnego modelu lifecycle. Każdy task powinien mieć właściciela, ścieżkę anulowania, sposób odebrania wyniku i określone zachowanie kolejki przy przeciążeniu. Obecne workery klipów i retencji są „fire and forget”.

`DashMap` jest używany bez guarda trzymanego przez `.await`, co jest dobre. Jednak check-then-act między mapą rezerwacji i filesystemem nadal jest wyścigiem logicznym.

### Async

I/O SQLx, plikowe Tokio i zewnętrzne requesty są asynchroniczne, a assembly jest w `spawn_blocking`. Brakuje tej samej dyscypliny dla OpenCV/ONNX. `thread::sleep` w dedykowanym workerze nie blokuje Tokio, ale uniemożliwia szybkie anulowanie.

### Granice domeny

Najczystsze fragmenty to config/storage oraz rozdzielenie HLS HTTP od katalogu danych. Najsłabsza granica jest wokół detekcji i clip lifecycle: istnieją dwa zestawy modeli detekcji, konfiguracja progu nie dociera do implementacji, a reguła rozszerzania zdarzeń nie ma jednego właściciela.

## Weryfikacja wykonana podczas audytu

| Polecenie | Wynik |
| --- | --- |
| `cargo metadata --no-deps --format-version 1` | OK; root workspace, trzy pakiety. |
| `cargo fmt --check` | OK. |
| `cargo check --workspace --all-targets` | OK. |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK. |
| `cargo test --workspace -- --test-threads=1` | NIESTABILNE; test HLS raz nie przeszedł (`404`, oczekiwano `200`), pozostałe wykonane wcześniej testy przeszły; dwa testy R2 były ignored. |
| `cargo test -p camwatch-server --test hls_test -- --nocapture` | OK przy powtórzeniu. |
| `cargo test -p camwatch-server -- --test-threads=1` | OK; 25 testów pakietu server przeszło. |

Testy integracyjne wymagające lokalnego Dockera, FFmpeg i GStreamera faktycznie wykonały się podczas pełnego runu i przeszły. Test prawdziwego Cloudflare R2 pozostał pominięty. Audyt nie potwierdza działania z fizyczną kamerą Tapo, długotrwałego soak testu, rzeczywistego PTZ ani prawdziwego R2.

## Rekomendowana kolejność napraw

### Etap 1 — poprawność danych

1. Usunąć TOCTOU między lease a retainerem i dodać deterministyczny test przeplotu.
2. Przekazać `motion_min_area` do MOG2 i dodać test dwóch progów.
3. Zaimplementować trigger rozszerzający `ended_at` oraz test długiego/seryjnego ruchu.
4. Dodać timeout assembly i test pipeline'u, który nie kończy się EOS.

### Etap 2 — kontrola zasobów i awarii

1. Zamienić unbounded channels na bounded i opisać politykę przeciążenia.
2. Nadać wszystkim workerom właściciela, token anulowania, `JoinHandle` i shutdown z deadline.
3. Usunąć `unwrap()` z produkcyjnej ścieżki runtime'u oraz raportować `JoinError`.
4. Wydzielić ograniczoną pulę MOG2/ONNX poza executor obsługujący HTTP.
5. Uczynić backoff RTSP przerwalnym.

### Etap 3 — spójność runtime i obserwowalność

1. Zdefiniować desired/applied state kamery i bezpieczny swap runtime'u.
2. Zachować klasyfikację błędów R2 oraz retry tylko dla błędów przejściowych.
3. Rozdzielić liveness, readiness i degraded status; dodać stan workerów i kolejki.
4. Dodać metryki długości kolejek, wieku najstarszego joba, zajętości dysku, reconnectów i dropped frames.

### Etap 4 — oczyszczenie architektury

1. Ujednolicić albo usunąć martwy `PersonDetector` i drugi model detekcji.
2. Wprowadzić wrapper sekretu z redagowanym `Debug`.
3. Rozbić `camera_routes.rs` i zawęzić publiczne pola `AppState`.
4. Uzgodnić dokumentację: osoba versus person/cat/dog, hash hasła, limity dysku, status tasków i faktyczne pokrycie testami.
5. Odizolować testy HTTP od GStreamera i dodać testy fault injection oraz concurrency.

## Kryterium ponownej oceny

Ocena gotowości może istotnie wzrosnąć dopiero po spełnieniu wszystkich poniższych warunków:

- brak znanych ścieżek utraty segmentu aktywnego klipu;
- konfiguracja wpływa na zachowanie dokładnie tak, jak opisuje UI i dokumentacja;
- każda kolejka jest ograniczona, a każdy worker ma kontrolowany lifecycle;
- błędy pojedynczej kamery, OpenCV, ONNX, R2 i assembly nie prowadzą do panic ani cichego zakończenia taska;
- pełny test suite przechodzi wielokrotnie bez flake;
- soak test 30–60 minut obejmuje 1–4 streamy, reconnect, długotrwały ruch, presję dysku i shutdown;
- osobno potwierdzono fizyczną kamerę, PTZ i prawdziwy R2.

Po tych zmianach architektura ma potencjał na solidne 7–8/10. W obecnym stanie jest dobrym, ambitnym MVP z wartościowymi testami, ale nie niezawodnym systemem monitoringu.
