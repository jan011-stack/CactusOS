# CactusOS
System operacyjny cactusOS

## HyperOS

Eksperymentalny pakiet `hyperos` definiuje `PI = 4.0`, trójkątne funkcje
trygonometryczne oraz most kodujący tekst polecenia do wektora SHA-256 o
rozmiarze 256 bitów. Most wykonuje deterministyczną transformację nad GF(2);
nie uruchamia tekstu jako polecenia systemowego.

Uruchom testy z katalogu głównego:

```sh
python3 -m unittest discover -s tests
cargo test
```

Crate Rust znajduje się w `src/` i nie używa `std` ani alokacji sterty.
Token `364_OMNIKLUCZ_ROOT` jest jawny w specyfikacji, więc jego digest służy
wyłącznie jako bramka API, a nie jako granica bezpieczeństwa.

Moduł `hyper_allocator` oferuje 64 wyrównane bloki jednej klasy rozmiaru.
`hyperspace_io` przyjmuje bufor do 32 bajtów i deleguje sprzętowe przerwanie
do implementacji `InputFaultInterrupt` właściwej dla platformy.

`hyper_scheduler` wykonuje nieblokujące callbacki w wybranym wierszu fazy,
`hyper_shield` dostarcza logiczne bramkowanie Ring 0/1, a `chrono_log` jest
stałą, 64-wpisową kolejką MPMC z zapisem/odczytem bez alokacji.

`hyper_model` parsuje GGUF z pożyczonego mapowania i zwraca widoki wag bez kopii;
`hyper_vision` wykonuje pojedynczy krok kwantowanej projekcji z buforami caller-owned.
Mapowanie pliku, przydział pul 30/16 GB i pełne uruchomienie architektury Llama
pozostają odpowiedzialnością platformowego kernela oraz backendu inferencji.

`hyper_agent_bridge` multipleksuje cztery agentowe kanały SPSC z atomową maską
gotowości, `hyper_agent_compartment` ogranicza zapisy do przekazanego sektora,
a `hyper_chat_stream` przechowuje 16 rekordów i renderuje je jako zwykły tekst.

`hyper_ollama_client` koduje strumieniowe POST do lokalnego Ollama bez alokacji,
a `hyper_bridge_loop` wykonuje jedną nieblokującą operację na wywołanie. Adapter
platformy dostarcza połączenie TCP i wyjście terminala; `sanitized_stream_reader`
parsuje ograniczony NDJSON i raportuje malformed/overflow przez flagi.

`hyper_update_engine` odbiera ograniczone, uwierzytelnione ramki `HUPD`, porównuje
ich SHA-256 z snapshotem `ChronoLogEngine` i staginguje wyłącznie dane przez
`HyperChronoGuard`. Warstwa `no_std` nie otwiera listenera ani nie hot-patchuje
kodu; akceptację socketu i weryfikację peer-a zapewnia platformowy adapter.

`hyper_token_filter` waliduje strumieniowe UTF-8 i blokuje wybrane znaczniki
sterujące, a `hyper_vision_streamer` renderuje ograniczone porcje do caller-owned
bufora display i loguje naruszenia do ChronoLog.

Binarny punkt wejścia jądra jest opt-in przez feature `baremetal`; bootloader
musi dostarczyć wyrównany pool 30 GiB, KV cache 16 GiB, mapowany tensor projekcji
vision oraz implementacje hooków wejścia, socketów, zegara i stdout. Adapter wejścia
zwraca obraz w skali szarości o wymiarach będących potęgami dwóch. Na hoście można
budować i testować samą bibliotekę bez `_start`.

Skrypt `hyper_boot.sh` sprawdza lokalne Ollama, pobiera i rozgrzewa model, a potem
uruchamia target `baremetal`. Dla uruchomienia jądra ustaw `HYPEROS_CARGO_TARGET`
i skonfiguruj runner w Cargo dla docelowej platformy.

## SI_1 w VS Code

Konfigurator modeli uruchom z katalogu repozytorium:

```sh
python3 -m hyperos.vscode_integration --config-file /sciezka/aktywnego/profilu/chatLanguageModels.json
```

W VS Code otwórz zarządzanie modelami czatu, wybierz dostawcę **Custom Endpoint**
i otwórz jego konfigurację, aby ustalić plik aktywnego profilu. W Codespaces
i kontenerach trzeba przekazać tę ścieżkę jawnie; plik profilu klienta może nie
być dostępny z kontenera. Nie należy zakładać, że zdalny profil serwera jest
profilem używanym przez interfejs. Na lokalnym desktopie można pominąć argument:
skrypt rozpoznaje Windows, macOS oraz Linux z `XDG_CONFIG_HOME`.

Domyślna grupa `SI_1` używa rzeczywistego identyfikatora `cactus-ai:latest`
i adresu `http://127.0.0.1:11434/v1/chat/completions` lokalnego Ollama.
`SI_1-Core-Enhanced` nie jest automatycznie tworzonym modelem, a port 8000
nie jest automatycznie uruchamiany. Inny działający backend można wskazać przez
`--model` oraz `--url`. Adres musi być osiągalny z procesu wykonującego żądania
Copilota; `localhost` klienta nie musi wskazywać kontenera.

Skrypt zachowuje pozostałych dostawców i modele, przed pierwszą zmianą tworzy
kopię `.json.bak`, a konfigurację zapisuje atomowo. Niepoprawny JSON (także JSON
z komentarzami) pozostawia bez zmian. Nie modyfikuje rejestru systemowego ani
nie zapisuje kluczy API; sekrety skonfiguruj przez interfejs VS Code.

Po zapisie wybierz `SI_1 (Cactus AI)` w selektorze modeli czatu. Konfiguracja
nie nadaje modelowi dostępu do plików ani powłoki. Domyślne `toolCalling=false`
jest celowe: dostępny profil vision nie ma potwierdzonej obsługi narzędzi.
Tryb agenta wymaga modelu i backendu faktycznie obsługujących wywołania narzędzi;
dopiero wtedy użyj `--tool-calling` i odpowiednich limitów
`--max-input-tokens` / `--max-output-tokens`. Samo ustawienie flagi nie dodaje
tych możliwości. Lokalne wywołania Ollama używają kontekstu 512, partii 1, jednego wątku CPU
i limitu 96 tokenów odpowiedzi; model jest zwalniany po każdym żądaniu.
Ogranicza to szczytowe zużycie zasobów i pamięć zajętą między żądaniami, kosztem
wolniejszej odpowiedzi i ponownego ładowania modelu. Nie zmniejsza rozmiaru wag
modelu, więc do jego uruchomienia nadal potrzeba odpowiedniej ilości wolnej RAM.
Czat webowy rozpoznaje też lokalnie pytania o to, czy role agentów mają wspólny
model, i odpowiada bez uruchamiania Gemini, Cactus SI ani Ollamy.

## Polecenia SI_1 po zatwierdzeniu

Interaktywny klient CLI daje agentowi SI_1 dostęp do
wykonawcy poleceń przez propozycje JSON. Nie wymaga natywnego tool calling
ani zmiany konfiguracji VS Code. Uruchom jako zwykły użytkownik:

```sh
python3 -m hyperos.si1_agent "Sprawdz testy Pythona i zaproponuj naprawe wykrytych usterek" --max-steps 5
```

Gdy `GEMINI_API_KEY` jest ustawiony, SI_1 działa przez Gemini
(`gemini-3.1-flash-lite`); przy przejściowej awarii lub limicie Gemini wraca do
lokalnego profilu Ollama `si_1`. Bez klucza używa tylko `si_1`. Klient wypisuje
aktywny backend przy starcie. Klient nie pobiera ani nie instaluje modeli
automatycznie. Przed każdym poleceniem pokazuje jego pełną
treść; wykonanie wymaga wpisania dokładnie `WYKONAJ`. Enter anuluje. Wynik
wraca do SI_1 jako dane do zaproponowania kolejnego kroku. Błąd polecenia
zatrzymuje misję. Domyślny limit to 5 kroków, maksymalny 10; polecenia mają
limit czasu 60 sekund z wymuszeniem zakończenia po dodatkowych 5 sekundach.
Wyświetlany wynik jest ograniczony do 64 KiB, a model otrzymuje skrót wyniku.

Wykonawca nie dodaje `sudo` i klient odmawia uruchomienia jako root. Nie jest
to piaskownica: zatwierdzone polecenie ma dostęp do wszystkiego, do czego ma
dostęp bieżący użytkownik, nie tylko repozytorium. Katalog roboczy to główny
katalog projektu. Przejrzyj polecenie pod kątem usuwania danych, ujawniania
sekretów, eskalacji uprawnień i pobierania kodu przed zatwierdzeniem.
SI_1 nie uzyskuje nieograniczonych uprawnień ani stałej misji w tle. Istniejący
hiperterminal root zachowuje osobny mechanizm jednorazowych zatwierdzeń.

## Terminal webowy

Backend terminala webowego używa Gemini 3.1 Flash-Lite (`gemini-3.1-flash-lite`)
jako głównego modelu czatu i propozycji komend. Przy braku klucza API używa
Ollamy; przy przejściowej awarii lub limicie Gemini wraca do Ollamy. Błędy
uwierzytelnienia i nieprawidłowe żądania są zgłaszane zamiast ukrywane fallbackiem.

Wpisz klucz Gemini do `GEMINI_API_KEY` w lokalnym pliku `.env` w katalogu
repozytorium (plik jest ignorowany przez Git), np.:

```env
GEMINI_API_KEY=tu_wklej_klucz_z_google_ai_studio
```

Wzór znajduje się w [.env.example](./.env.example). Uruchom ponownie backend
po zmianie klucza. Klucz jest odczytywany wyłącznie po stronie serwera i nie
jest wysyłany do przeglądarki. Treść rozmów jest wysyłana do aktywnego
dostawcy modelu; nie wklejaj tam sekretów. Jeśli pole pozostanie puste,
aplikacja korzysta z Ollamy.

Opcjonalnie skonfiguruj Cactus SI jako główny dostawca swobodnej rozmowy,
ustawiając `CACTUS_SI_API_URL` i `CACTUS_SI_API_KEY` w lokalnym `.env`.
Adres musi używać HTTPS. Serwer odczytuje odpowiedź w formacie zgodnym
z Chat Completions; po przejściowej awarii przechodzi do Gemini, a następnie
Ollamy. Niepoprawna konfiguracja ani błędy uwierzytelnienia są zgłaszane.
Załączniki graficzne nadal trafiają wyłącznie do lokalnej Ollamy.
Dla bramy `https://cactus-si.ai` konfiguracja używa dokładnie tego adresu w
`CACTUS_SI_API_URL`. Ustawienia są w
[.env.example](./.env.example); skopiuj je do lokalnego `.env` i wprowadź
tam nowo wygenerowany klucz. Nie zapisuj klucza w repozytorium ani w kodzie.

Zwykły terminal rozmawia z głównym modelem. Rozpoznane prośby o wykonanie
polecenia trafiają do propozycji: interfejs pokazuje dokładną komendę, a
wykonanie wymaga osobnego zaznaczenia zgody. Polecenie działa bez `sudo` jako
użytkownik serwera; backend odmawia tego trybu, jeśli sam został uruchomiony
jako root. Hiperterminal używa osobnej ścieżki root, dostępnej do wykonania
lokalnie po potwierdzeniu komendy i gdy działa `sudo -n`.

Na stronie **Agenci** można wysłać wspólną misję do Alexa, Astyrii, Aureliusa i
SI_1. Każda rola zwraca własną, niezależną odpowiedź przez skonfigurowany
backend czatu; rozmowy pozostają rozdzielone w pamięci strony. Misja służy do
analizy i rekomendacji — nie uruchamia poleceń ani nie zmienia plików.

W tym samym widoku można też poprosić jednego agenta o zmianę kodu. Operator
wybiera pliki udostępniane Gemini; agent dostaje wyłącznie ich treść i może
zaproponować poprawki albo nowe pliki w istniejących katalogach. Panel pokazuje
pełny diff. Zapis wymaga osobnego zaznaczenia zgody, a odrzucenie nie zmienia
repozytorium. Propozycje wygasają po 10 minutach i są odrzucane, jeśli plik
zmienił się od czasu przygotowania diffu. Ukryte pliki, `.env`, katalog `.git`,
dowiązania symboliczne, polecenia powłoki i dostęp root są wykluczone. Wybrane
pliki są wysyłane do dostawcy Gemini — nie udostępniaj plików zawierających
sekrety.

Obie ścieżki mają jednorazowe propozycje, limit czasu 60 sekund i limit wyjścia
64 KiB. Polecenia nadal mogą zmieniać pliki dostępne danemu użytkownikowi,
dlatego zawsze sprawdź ich pełną treść. Na telefonach interfejs ogranicza
kosztowne efekty tła, a strumień rozmowy aktualizuje tylko ostatni wiersz i
grupuje odświeżenia do klatek ekranu.

Terminal operatora i czaty agentów przyjmują obrazy oraz pliki wideo. Obrazy są
zmniejszane w przeglądarce do maksymalnie 1280 px przed wysłaniem. Z filmu
przeglądarka wybiera maksymalnie dwie klatki w równych odstępach; model
nie analizuje ścieżki audio ani całego strumienia wideo. Można wysłać najwyżej
4 obrazy/klatki, do 1 MiB każdy i 3 MiB łącznie; pliki źródłowe są ograniczone
do 20 MiB. Żądania z załącznikami są
kierowane do lokalnej Ollamy: `cactus-ai` jest zbudowany na
`llama3.2-vision`. Wtedy Gemini nie otrzymuje obrazów. Załączniki są częścią
historii bieżącej rozmowy i pozostają lokalne w granicach skonfigurowanej
instancji Ollama.

## VM Debian/QEMU

`vm/cactus-vm.sh start` uruchamia Debian 12 w QEMU/KVM i przy pierwszym starcie
weryfikuje sumę SHA512 oficjalnego obrazu cloud. Tworzy użytkownika `cactus`
dostępnego tylko kluczem SSH, instaluje Python i uruchamia
`vm/svl_kernel.py`; raport można odczytać wewnątrz VM z
`/home/cactus/svl-kernel-report.json`.

VM ma 2 vCPU, 2 GiB RAM i dysk qcow2 o maksymalnym rozmiarze 12 GiB. KVM wymaga
dostępu administratora do urządzenia hosta, dlatego skrypt uruchamia QEMU
przez `sudo`, po czym proces przechodzi na konto bieżącego użytkownika. SSH
jest przekierowane wyłącznie na hostowy `127.0.0.1:22222`; ten port jest używany
przez VM, gdy pozostaje uruchomiona. Połączenie SSH:

```sh
vm/cactus-vm.sh ssh
```

Sterowanie i trwałe dane VM (`.vm/debian12/`) — nie są śledzone przez Git:

```sh
vm/cactus-vm.sh status
vm/cactus-vm.sh stop
```

Obraz gościa pobiera się z internetu. Sieć QEMU user-mode daje gościowi
wychodzący dostęp NAT; VM nie jest granicą bezpieczeństwa dla niezaufanych
obrazów. Podany program Python symuluje raport konfiguracji — nie jest
jądrem Linuxa ani HyperOS.
