# Camwatch — specyfikacja backendu SSR

## Cel dokumentu

Ten dokument jest zleceniem implementacyjnym dla backendu Camwatch. Opisuje docelową architekturę, granice crate'ów, widoki SSR, logowanie, API między warstwami, zasady bezpieczeństwa i kolejność prac.

Nie należy zmieniać decyzji opisanych niżej bez potwierdzenia właściciela projektu.

## Decyzje już podjęte

- Backend HTTP jest osobnym crate'em.
- Obecny crate `camwatch` jest importowaną biblioteką z logiką domenową i integracjami.
- UI jest renderowane po stronie serwera.
- Nie używamy Reacta, Next.js, SPA, WASM ani hydration.
- Podstawowy stack SSR to `axum` + `askama`.
- `htmx` jest częścią panelu i służy do częściowego odświeżania HTML.
- Nie prowadzimy historii zdarzeń ani uploadów.
- Lifecycle klipu i uploadu działa wyłącznie w pamięci procesu.
- SQLite przechowuje kamery i segmenty bufora, ale nie eventy ani uploady.
- PTZ nie ma osobnego traita `PtzController`. Możliwości PTZ są sprawdzane przy starcie, a sterowanie jest wykonywane przez `OnvifConnection` ukrytą wewnątrz crate'a `camwatch`.
- R2 jest opcjonalne. Worker wykonuje maksymalnie trzy próby uploadu; po restarcie procesu nie ma trwałej kolejki ani wznowienia.
- Sesje użytkowników są in-memory i po restarcie aplikacji użytkownik loguje się ponownie.
- Zapis edycji kamery od razu przeładowuje wyłącznie runtime tej kamery.
- Panel ma obsługiwać HLS w Safari, Chrome i Firefox przez lokalnie serwowany `hls.js`.
- Gdy oba envy logowania nie są ustawione, lokalny panel używa domyślnych danych `admin` / `admin`.

## Wybór SSR

### Rekomendacja

Użyć `axum` jako serwera HTTP i routera oraz `askama` jako silnika szablonów.

- Axum działa natywnie z Tokio, ma routing, extractory, middleware Tower oraz naturalny model współdzielonego stanu.
- Askama renderuje typowane szablony podobne do Jinja podczas kompilacji. Nie wymaga JavaScriptu, bundlera ani osobnej aplikacji frontendowej.
- `askama_web` zapewnia bezpośrednią integrację odpowiedzi Askama z Axum.

Źródła:

- [Axum](https://docs.rs/axum/latest/axum/)
- [Askama](https://askama.rs/en/stable/doc/askama/index.html)
- [Askama Web dla Axum](https://docs.rs/askama_web/latest/askama_web/)

### Dlaczego nie Leptos

Leptos jest sensownym, pełnym frameworkiem SSR w Rust i może działać jako MPA. Wprowadza jednak własny model komponentów, reaktywności i opcjonalnej hydracji. Dla panelu administracyjnego Camwatch, z klasycznymi stronami, formularzami i kilkoma przyciskami PTZ, byłby dodatkową warstwą bez wyraźnej korzyści.

Nie należy go dodawać do tego zadania.

### htmx w panelu

Panel używa `htmx` do dynamicznych interakcji, ale pozostaje aplikacją SSR:

- pierwsze wejście na trasę zawsze zwraca pełną stronę HTML z Askama;
- htmx pobiera wyłącznie fragmenty HTML i podmienia je w DOM;
- serwer nie zwraca JSON dla własnego UI;
- nie ma osobnej aplikacji frontendowej, bundlera ani stanu klienta;
- formularze i linki powinny mieć zwykły wariant HTTP jako fallback, gdy JavaScript jest wyłączony.

htmx obsługuje odświeżanie kart kamer i statusu, wysyłanie formularzy oraz przyciski PTZ. [Dokumentacja htmx](https://htmx.org/docs/)

## Docelowy układ workspace

```text
.
├── Cargo.toml
├── config/
├── docs/
└── crates/
    ├── camwatch/
    │   ├── src/
    │   │   ├── application/
    │   │   ├── bucket/
    │   │   ├── clips/
    │   │   ├── config/
    │   │   ├── motion/
    │   │   ├── onvif/
    │   │   ├── runtime/
    │   │   ├── storage/
    │   │   └── stream/
    │   └── Cargo.toml
    └── camwatch-server/
        ├── src/
        │   ├── auth/
        │   ├── routes/
        │   ├── views/
        │   ├── app_state.rs
        │   ├── error.rs
        │   ├── router.rs
        │   └── main.rs
        ├── templates/
        ├── static/
        └── Cargo.toml
```

W rootowym `Cargo.toml` należy dodać `crates/camwatch-server` do `workspace.members`.

`camwatch-server` ma zależność pathową na `camwatch`:

```toml
camwatch = { path = "../camwatch" }
```

## Odpowiedzialność crate'ów

### `camwatch`

Crate biblioteczny. Nie może zależeć od Axum, Askama, HTML, ciasteczek, sesji ani szczegółów HTTP.

Zawiera:

- ładowanie i walidację konfiguracji runtime'u;
- otwarcie SQLite i bootstrap kamer;
- uruchomienie runtime'ów kamer;
- RTSP, segmenty, detekcję ruchu i YOLO;
- składanie klipów;
- worker R2 i retencję segmentów;
- odczyt możliwości PTZ przy uruchamianiu kamery;
- typy i funkcje infrastruktury używane przez backend;
- modele storage oraz runtime kamer.

Crate pozostaje wolny od Axum, Askama, HTML, ciasteczek, sesji i szczegółów HTTP.

### `camwatch-server`

Crate binarny. Odpowiada wyłącznie za HTTP i prezentację:

- parsowanie CLI serwera i uruchomienie logowania;
- bezpośredni bootstrap zasobów `camwatch` i trzymanie ich w stanie aplikacji;
- Axum router, middleware, uwierzytelnienie i sesje;
- renderowanie pełnych stron oraz fragmentów HTML dla htmx;
- serwowanie własnych statycznych assetów CSS/JS;
- mapowanie błędów domenowych na odpowiedzi HTTP;
- nieujawnianie sekretów i danych wewnętrznych runtime'u.

## Bootstrap aplikacji

`camwatch` jest wyłącznie biblioteką. `camwatch-server` jest composition rootem procesu i bezpośrednio tworzy `Database`, workery, `ClipManager` oraz `CameraRuntime`. Stan tych zasobów trafia do `AppState`.

Nie dodajemy osobnej warstwy `CamwatchService` ani `CamwatchHandle`. Backend może korzystać z publicznych typów crate'a `camwatch`, a logika prezentacji pozostaje w handlerach i modelach widoku.

`CameraSummary` i `CameraDetails` są modelami odczytu dla UI. Nie mogą być bezpośrednimi modelami SQLx ani GStreamera.

Minimalne dane widoczne z backendu:

- `id` i nazwa kamery;
- status `online` lub `offline` oraz czas ostatniej zmiany;
- flaga `ptz_available`;
- ścieżka lub identyfikator playlisty HLS, gdy VID-04 będzie gotowe.

### Mapa runtime'ów kamer

SQLite pozostaje źródłem prawdy dla listy i konfiguracji kamer. `AppState` przechowuje wyłącznie mapę aktualnie uruchomionych runtime'ów, indeksowaną identyfikatorem kamery.

Wpis runtime'u powinien:

- posiadać `CancellationToken` do graceful shutdownu;
- posiadać `JoinHandle` do oczekiwania na zakończenie taska;
- przechowywać wynik wykrycia PTZ z momentu startu;
- pozwalać stwierdzić, czy task nadal działa.

Po usunięciu lub przeładowaniu kamery server anuluje jej runtime i czeka na zakończenie taska przed uruchomieniem nowego. Przy zamknięciu procesu wszystkie runtime'y są zatrzymywane tą samą ścieżką.

Modele odczytu dla UI powstają na żądanie przez połączenie danych z SQLite, statusu z `CameraStatusModel` oraz informacji z mapy runtime'ów. Nie tworzymy osobnego rejestru duplikującego dane kamer.

Server może korzystać bezpośrednio z publicznych typów `camwatch`, ale handler nie powinien wykonywać przypadkowych operacji infrastrukturalnych. Operacje PTZ i CRUD mają być skupione w modułach backendu.

### CRUD kamer przez panel

CRUD kamer jest częścią docelowego panelu. Przy każdym starcie wpisy kamer obecne w TOML są upsertowane do SQLite, a następnie SQLite jest źródłem prawdy dla kamer widocznych oraz edytowanych przez UI. Kamery nieobecne w TOML pozostają w bazie.

`CameraInput` powinien obejmować:

- identyfikator kamery;
- nazwę;
- zaszyfrowany URL RTSP obsługiwany przez backend;
- kodek RTSP;
- URL ONVIF oraz zaszyfrowane dane ONVIF, jako parę opcjonalną;
- próg pola ruchu;
- próg pewności YOLO;
- `clip_after_motion`.

Formularz nie przyjmuje sekretów RTSP ani ONVIF. Przyjmuje wyłącznie nazwy zmiennych środowiskowych, które są walidowane tak samo jak konfiguracja TOML.

Po utworzeniu, edycji lub usunięciu kamery server ma wykonać tę operację w SQLite oraz od razu zsynchronizować runtime bez restartu całej aplikacji:

1. zwalidować wejście;
2. zapisać zmianę w bazie;
3. zatrzymać poprzedni runtime danej kamery, gdy istnieje;
4. utworzyć nowy runtime z danych zapisanych w SQLite;
5. zaktualizować rejestr kamer oraz status widoczny przez UI;
6. zwrócić kontrolowany błąd, jeśli runtime nie może zostać uruchomiony.

Usunięcie kamery powinno być soft-delete zgodne z istniejącym `deleted_at`, zatrzymać runtime i usunąć kamerę z rejestru widocznego w panelu. Nie usuwać automatycznie plików segmentów ani lokalnych klipów bez osobnej decyzji.

### PTZ w pierwszej wersji

Obecne `cam_move` wykonuje bezpieczny ruch przez około 200 ms, a następnie wysyła `Stop`.

Pierwsza wersja UI ma mieć cztery przyciski: góra, dół, lewo, prawo. Jedno kliknięcie oznacza jeden krótki ruch. Nie implementować sterowania typu „przytrzymaj przycisk”, dopóki nie powstanie osobny, przetestowany model `start`/`stop` i ograniczanie równoległych komend.

Jeżeli kamera nie ma PTZ, kontrolki są ukryte, a endpoint nadal odmawia komendy odpowiedzią kontrolowaną przez serwer.

## Backend HTTP i SSR

### Zależności serwera

Docelowo potrzebne będą co najmniej:

- `axum`;
- `askama`;
- `askama_web` z integracją Axum;
- `tower-http` dla request tracingu, bezpiecznych nagłówków i serwowania assetów;
- `tower-sessions` oraz in-memory store sesji;
- crate do kryptograficznego generowania identyfikatorów sesji, jeżeli nie zapewni go wybrany store;
- `subtle` lub równoważny mechanizm stałoczasowego porównania hasła.

Wersje należy wybrać zgodnie z aktualnym, kompatybilnym ekosystemem Axum podczas implementacji. Nie dopisywać przestarzałych bibliotek sesji tylko dlatego, że pasują do starego przykładu z Internetu.

### `AppState`

Stan Axum powinien być mały i klonowalny:

```rust
pub struct AppState {
    pub database: Arc<Database>,
    pub clip_manager: Arc<ClipManager>,
    pub status_model: Arc<CameraStatusModel>,
    pub auth: Arc<AuthService>,
}
```

`AppState` nie zawiera haseł w postaci możliwej do zalogowania. Zawiera bezpośrednio zasoby `camwatch`, ponieważ server jest composition rootem.

### Routing

Publiczne endpointy:

| Metoda | Ścieżka | Rola |
| --- | --- | --- |
| GET | `/login` | Formularz logowania |
| POST | `/login` | Weryfikacja danych i utworzenie sesji |
| GET | `/assets/*path` | CSS oraz ewentualny mały JS lokalny |

Endpointy wymagające sesji:

| Metoda | Ścieżka | Rola |
| --- | --- | --- |
| GET | `/` | Redirect do `/cameras` |
| GET | `/cameras` | Lista kamer |
| GET | `/cameras/new` | Formularz dodania kamery |
| POST | `/cameras` | Utworzenie kamery |
| GET | `/cameras/:camera_id` | Szczegóły kamery |
| GET | `/cameras/:camera_id/edit` | Formularz edycji kamery |
| POST | `/cameras/:camera_id` | Zapis edycji kamery |
| POST | `/cameras/:camera_id/delete` | Soft-delete kamery i zatrzymanie runtime'u |
| POST | `/cameras/:camera_id/ptz/:direction` | Krótki ruch PTZ |
| POST | `/logout` | Unieważnienie sesji i redirect do `/login` |

Endpointy fragmentów htmx:

| Metoda | Ścieżka | Rola |
| --- | --- | --- |
| GET | `/fragments/cameras` | Karty kamer bez pełnego layoutu |
| GET | `/fragments/cameras/:camera_id/status` | Status pojedynczej kamery |

Handler PTZ oraz formularze edycji kamery rozpoznają nagłówek `HX-Request`: dla htmx zwracają zaktualizowany fragment HTML, a dla zwykłego formularza wykonują redirect po POST.

Nie tworzyć publicznego REST API ani odpowiedzi JSON wyłącznie po to, aby zasilać własny SSR. Jeśli kiedyś pojawi się zewnętrzna integracja, API będzie osobnym zadaniem i osobnym kontraktem.

## Widoki

### 1. Logowanie — `/login`

Elementy:

- login;
- hasło;
- komunikat „Nieprawidłowy login lub hasło” bez wskazywania, które pole było błędne;
- brak szczegółów konfiguracji, stack trace'ów i informacji o kamerach.

Zalogowany użytkownik, który otworzy `/login`, ma zostać przekierowany do `/cameras`.

### 2. Lista kamer — `/cameras`

To jest główny dashboard MVP.

Każda karta kamery zawiera:

- nazwę;
- identyfikator techniczny;
- status online/offline;
- czas ostatniej zmiany statusu;
- oznaczenie PTZ, jeżeli jest dostępne;
- link do szczegółów.

Na stronie można dodać lekki ogólny status procesu: liczba kamer online/offline oraz status R2 w bieżącym procesie. Nie tworzyć tabeli historii zdarzeń.

### 3. Szczegóły kamery — `/cameras/:camera_id`

Elementy:

- breadcrumb lub link powrotu do listy;
- nazwa i status kamery;
- obszar live preview HLS, gdy VID-04 będzie gotowe;
- komunikat o niedostępnym strumieniu;
- panel PTZ z czterema przyciskami tylko dla kamer z `ptz_available = true`;
- czytelny komunikat po błędzie PTZ.

Nie wyświetlać historii eventów, archiwum klipów ani listy dawnych uploadów.

### 4. Dodanie i edycja kamery — `/cameras/new`, `/cameras/:camera_id/edit`

Formularz zawiera wszystkie pola `CameraInput`, a po zapisie zwraca użytkownika do szczegółów kamery lub listy kamer.

Walidacja jest wykonywana po stronie serwera. Błędne dane powodują ponowne wyrenderowanie pełnej strony formularza z błędami.

Formularz nie pokazuje zapisanych wartości sekretów. Pola sekretów są zapisywane po stronie serwera w zaszyfrowanej postaci.

Usunięcie kamery wymaga osobnego formularza POST z tokenem CSRF i potwierdzeniem w UI.

### 5. Strony błędów

Potrzebne są także renderowane SSR:

- 404 dla nieistniejącej kamery;
- 403 dla braku autoryzacji lub niepoprawnego tokenu CSRF;
- 409 dla PTZ niedostępnego dla danej kamery;
- 500 z generycznym komunikatem dla nieoczekiwanego błędu.

Szczegóły błędu trafiają wyłącznie do `tracing`, nigdy do HTML.

## Szablony i modele widoku

Askama ma renderować pliki z `crates/camwatch-server/templates/`.

Proponowany układ:

```text
templates/
├── layouts/
│   └── base.html
├── auth/
│   └── login.html
├── cameras/
│   ├── index.html
│   └── show.html
├── components/
│   ├── camera_card.html
│   ├── camera_status.html
│   ├── camera_activity.html
│   └── ptz_controls.html
└── errors/
    ├── forbidden.html
    ├── not_found.html
    └── internal.html
```

Zgodnie z konwencją projektu każdy publiczny `struct`, `enum` i jego implementacja mają własny plik. Dotyczy to również modeli widoku, np.:

```text
src/views/
├── camera_activity_view.rs
├── camera_details_view.rs
├── camera_list_item_view.rs
├── camera_list_view.rs
├── login_view.rs
└── ptz_controls_view.rs
```

Szablon nie może wykonywać logiki domenowej. Handler pobiera dane z `AppState`, buduje prosty model widoku, a Askama tylko go renderuje.

## Logowanie i sesje

### Konfiguracja użytkownika

Wymagane są dwa sekrety środowiskowe:

```text
CAMWATCH_USER_LOGIN
CAMWATCH_USER_PASSWORD
```

Wartości sekretów są przechowywane w konfiguracji jako wersjonowane ciphertexty AES-256-GCM zakodowane Base64. Klucz `CAMWATCH_CONFIG_KEY` jest dostarczany poza plikiem konfiguracyjnym.

Zasady:

- gdy oba envy są nieustawione, użyć lokalnego fallbacku `admin` / `admin`;
- gdy ustawione jest tylko jedno z dwóch env, serwer kończy start czytelnym błędem;
- pusta wartość ustawionego env powoduje błąd startu bez wypisania sekretu;
- wartości są odczytywane raz przy starcie;
- nie wolno implementować fallbacku do TOML, SQLite ani argumentów CLI;
- login i hasło nie mogą trafić do `Debug`, `Display`, logów, komunikatów HTTP ani test snapshots.

Domyślne `admin` / `admin` jest dopuszczalne wyłącznie dlatego, że panel MVP bezwzględnie binduje się do loopbacka. Nie wolno później umożliwić bindu do LAN bez usunięcia fallbacku albo wymuszenia ustawionych env.

### Czy hashowanie hasła jest potrzebne?

Nie w obecnym modelu.

Hasła panelu nadal pochodzą z konfiguracji środowiska i nie są częścią szyfrowanego TOML-u. Hasło nie jest zapisywane w bazie; domyślny fallback `admin/admin` pozostaje ograniczony do lokalnego developmentu.

Nadal należy:

- porównywać hasło w czasie stałym;
- nie logować hasła;
- przy ustawionym `CAMWATCH_USER_PASSWORD` zalecać silne hasło, ponieważ chroni ono panel;
- używać HTTPS przy dostępie spoza localhost;
- nie przechowywać identyfikatorów sesji w `localStorage` ani w URL.

### Sesja

Użyć sesji serwerowej w pamięci procesu, zarządzanej przez `tower-sessions` z in-memory store.

Przeglądarka dostaje wyłącznie losowy, nieprzezroczysty identyfikator sesji w ciasteczku. Stan sesji zostaje po stronie serwera. Restart aplikacji celowo unieważnia wszystkie sesje; użytkownik po restarcie loguje się ponownie. Jest to zgodne z podejściem in-memory i nie wymaga trzeciego sekretu środowiskowego.

Sesja wygasa po jednej godzinie bezczynności. Każde poprawne, uwierzytelnione żądanie może odnowić licznik bezczynności do jednej godziny.

Panel MVP jest dostępny wyłącznie na loopbackie (`127.0.0.1` lub `::1`). Serwer ma odrzucać konfigurację wiążącą go z adresem LAN lub publicznym. W tym lokalnym trybie cookie nie może mieć flagi `Secure`, ponieważ przeglądarka nie wyśle go po zwykłym HTTP.

Wymagane parametry ciasteczka lokalnego:

- `HttpOnly`;
- `SameSite=Strict`;
- `Path=/`;
- bez atrybutu `Domain`;
- krótki TTL oraz unieważnienie przy logout.

Jeśli kiedyś panel zostanie wystawiony poza localhost, przed zmianą adresu bind należy wprowadzić HTTPS, ustawić `Secure` i użyć cookie z prefiksem `__Host-`.

Wymagane parametry ciasteczka w środowisku HTTPS:

- `HttpOnly`;
- `Secure`;
- `SameSite=Strict`;
- `Path=/`;
- bez atrybutu `Domain`;
- krótki TTL oraz unieważnienie przy logout.

Przy HTTPS użyć nazwy z prefiksem `__Host-`, np. `__Host-camwatch-session`.

`SameSite` jest tylko dodatkową ochroną. Wszystkie mutujące żądania POST, w tym logout i PTZ, muszą mieć token CSRF albo równoważną, świadomie zaimplementowaną ochronę.

Źródła:

- [OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
- [tower-sessions](https://docs.rs/tower-sessions/latest/tower_sessions/)
- [OWASP CSRF Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html)

### Przepływ logowania

1. Użytkownik otwiera `GET /login`.
2. Serwer renderuje formularz z tokenem CSRF.
3. `POST /login` odbiera login i hasło.
4. Serwer porównuje login oraz hasło, bez rozróżniania komunikatu błędu.
5. Przy sukcesie tworzy świeżą sesję, ustawia cookie i robi redirect do `/cameras`.
6. Przy błędzie renderuje formularz z generycznym komunikatem.
7. `POST /logout` unieważnia sesję, usuwa cookie i przekierowuje do `/login`.

Należy dodać limit prób logowania na IP lub przynajmniej opóźnienie po serii błędów. Jest to panel do kamer, więc brute force nie może być ignorowany tylko dlatego, że aplikacja działa w LAN.

## HLS i assety

Aktualny runtime nie udostępnia jeszcze HLS; widok kamery ma przygotować miejsce na tę funkcję, ale odtwarzacz zależy od taska VID-04.

Nie wystawiać całego katalogu `data/` przez `ServeDir`. W przyszłości HLS musi być serwowane przez ograniczone, uwierzytelnione endpointy, które mapują znane `camera_id` na właściwy katalog playlisty. Nie wolno przyjmować ścieżki pliku od użytkownika.

HLS ma różne wsparcie przeglądarek. Safari odtwarza je natywnie, natomiast Chrome i Firefox zwykle potrzebują małego klienta `hls.js`. `hls.js` jest wymaganym elementem panelu i ma być serwowany lokalnie; nie zmienia to architektury SSR ani nie tworzy SPA.

CSS i `hls.js` należy trzymać lokalnie w `crates/camwatch-server/static/`, bez CDN.

## Obsługa błędów i logowanie

- Błędy uruchomienia `camwatch` kończą start serwera czytelnym komunikatem administracyjnym, bez sekretów.
- Brak jednej kamery RTSP nie kończy działania backendu; kamera jest prezentowana jako offline.
- Błąd PTZ jest logowany po stronie serwera i pokazany użytkownikowi jako ogólny komunikat.
- Błąd R2 nie pokazuje credentiali, endpointu ani pełnych szczegółów SDK.
- Każdy request ma trace ID lub request ID w logu.
- Nie logować ciała formularza loginu, nagłówka `Cookie`, `Set-Cookie`, haseł, tokenów CSRF, adresów RTSP ani sekretów R2.

## Testy wymagane

### `camwatch`

- bootstrap servera buduje stan bez zależności HTTP w crate'cie `camwatch`;
- Rejestr kamer poprawnie listuje kamery i ich statusy.
- Utworzenie, edycja i soft-delete kamery synchronizują SQLite oraz rejestr runtime'u.
- Edycja jednej kamery nie zatrzymuje pozostałych runtime'ów.
- Kamera bez PTZ nie otrzymuje możliwości sterowania.
- Komenda PTZ jest serializowana dla jednej kamery.
- Lifecycle klipu/uploadu pozostaje in-memory i nie tworzy tabel eventów/uploadów.

### `camwatch-server`

- niezalogowany użytkownik jest przekierowany do `/login` dla każdej chronionej ścieżki;
- poprawne dane tworzą sesję, niepoprawne nie tworzą;
- brak obu env logowania pozwala na lokalne logowanie `admin` / `admin`;
- ustawienie tylko jednego env logowania kończy start bez ujawnienia wartości;
- cookie ma właściwe atrybuty w lokalnym trybie loopback;
- sesja wygasa po godzinie bezczynności;
- logout unieważnia sesję;
- formularze POST odrzucają niepoprawny token CSRF;
- lista kamer renderuje HTML z danymi testowego stanu aplikacji;
- formularze dodania i edycji kamery renderują błędy walidacji bez utraty danych jawnych;
- nieistniejąca kamera zwraca SSR 404;
- PTZ dla kamery bez PTZ jest odrzucone;
- błąd PTZ renderuje bezpieczny komunikat;
- widok kamery używa natywnego HLS w Safari i lokalnego `hls.js` w Chrome oraz Firefox;
- odpowiedzi i logi testowe nie zawierają loginu, hasła ani wartości cookie.

Testy HTTP powinny działać bez GStreamera, kamery, ONVIF, R2 ani prawdziwej bazy danych. Integracje runtime'u pozostają w testach crate'a `camwatch`.

## Kolejność implementacji

1. Dodać `camwatch-server` do workspace'u z pustym `GET /health`.
2. Przenieść bootstrap obecnego `crates/camwatch/src/main.rs` bezpośrednio do `camwatch-server` i umieścić zasoby w `AppState`.
3. Dodać in-memory rejestr kamer i odczytowe modele UI w crate'ie `camwatch`.
4. Dodać Axum router, `AppState`, obsługę błędów i bazowy layout Askama.
5. Dodać logowanie, sesje, middleware autoryzacji, CSRF oraz logout.
6. Zaimplementować `/cameras` i `/cameras/:camera_id` bez HLS.
7. Dodać CRUD kamer w SQLite wraz z kontrolowanym przeładowaniem pojedynczego runtime'u.
8. Udostępnić bezpieczny endpoint PTZ jako krótkie ruchy.
9. Dodać endpointy htmx do odświeżania statusów, aktywności, formularzy i odpowiedzi PTZ.
10. Po ukończeniu VID-04 dodać chronione HLS do widoku kamery.
11. Osobno wykonać testy R2 opisane w backlogu.

Po każdym etapie uruchomić co najmniej `cargo check`, właściwe testy crate'a, `cargo clippy -- -D warnings` i `git diff --check`.

## Poza zakresem pierwszego wdrożenia

- historia zdarzeń i uploadów;
- archiwum klipów;
- trwałe sesje i automatyczne odtwarzanie sesji po restarcie;
- wielu użytkowników, role i reset hasła;
- zewnętrzne OAuth/OIDC;
- publiczne REST API;
- ciągłe sterowanie PTZ przy przytrzymaniu przycisku;
- CDN dla assetów;
- aplikacja mobilna.

## Otwarte pytania do decyzji

Brak decyzji blokujących implementację backendu.
