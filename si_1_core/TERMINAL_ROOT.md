# Terminal + hiperterminal root — SI_1 fix notes

## Co NIE zostało zmienione (celowo)

- Brak automatycznego roota bez zgody operatora
- Checkbox / WYKONAJ nadal wymagane
- CSRF + lokalny Host nadal wymagane dla wykonania z UI
- `HyperSpaceTerminal` w Rust **nie wykonuje** poleceń — tylko wektor 256-bit

## Dlaczego „polecenia nie działają”

| Objaw | Przyczyna | Naprawa |
|-------|-----------|---------|
| Status: ROZMOWA DOSTĘPNA · WYKONANIE ROOT TYLKO LOKALNIE | `Host` nie jest lokalny albo brak CSRF | Uruchom `python3 -m hyperos.web_server` lokalnie, otwórz 127.0.0.1 |
| root_available=false | `sudo -n id -u` ≠ 0 | Skonfiguruj NOPASSWD (poniżej) |
| Backend root niepodłączony | Brak meta terminal-csrf | Serwer nie wstawił CSRF — restart web_server |
| Polecenie user działa, root nie | Brak passwordless sudo | sudoers |
| Hardcoded `/workspaces/CactusOS` w promptach modelu | Codespaces path | WORKSPACE w kodzie jest dynamiczny; prompt może mylić model |

## Bezpieczne włączenie root (lokalny operator)

**Nie** dawaj `ALL=(ALL) NOPASSWD: ALL` na produkcji publicznej.

Minimalny wariant (tylko test lokalny):

```bash
# Sprawdź diagnostykę
python3 -m si_1_core.terminal_fix --diagnose

# Przykład sudoers (wymaga Twojej decyzji jako operatora):
# echo 'TWOJ_USER ALL=(root) NOPASSWD: /usr/bin/timeout, /bin/bash' | sudo tee /etc/sudoers.d/cactus-terminal
# sudo chmod 440 /etc/sudoers.d/cactus-terminal
# sudo visudo -cf /etc/sudoers.d/cactus-terminal
```

Po konfiguracji:

```bash
sudo -n -- /usr/bin/id -u   # musi wypisać 0
python3 -m si_1_core.terminal_fix --diagnose
```

## Test zatwierdzonego wykonania

```bash
# user
python3 -m si_1_core.terminal_fix --approve --command 'whoami; pwd'

# root (tylko gdy root_available)
python3 -m si_1_core.terminal_fix --approve --root --command 'id -u'
```

Bez `--approve` komenda **zawsze** jest odrzucana.

## Integracja z UI

1. Terminal zwykły → `/api/terminal/propose` → zgoda → `/api/terminal/resolve` → `execute_command(elevated=False)`
2. Hiperprzestrzeń → wektor 16×16 + `/api/root/propose` → zgoda → `/api/root/resolve` → `execute_command(elevated=True)` = `sudo -n`

SI_1 przygotowuje komendy; **wykonanie** nadal należy do operatora.
