# Przesyłanie plików w terminalu CactusOS

System może odczytywać pliki z katalogu `uploads/` i generować na ich podstawie.

## Uruchomienie
```bash
bash scripts/file_transfer.sh
```
Serwer: http://127.0.0.1:8765/ (tylko localhost)

## Przesyłanie
**Przeglądarka:** otwórz http://127.0.0.1:8765/ i wybierz plik.

**Terminal (curl):**
```bash
curl -F 'file=@dokument.txt' http://127.0.0.1:8765/upload
```

**JSON (base64):**
```bash
curl -X POST http://127.0.0.1:8765/upload \
  -H 'Content-Type: application/json' \
  -d '{"filename":"test.txt","content_b64":"dGVzdA=="}'
```

## Lista plików
```bash
curl http://127.0.0.1:8765/files
```

## Użycie przez system
Pliki lądują w `uploads/`. Silnik / terminal może je czytać:
```bash
cat uploads/dokument.txt
python3 -c "print(open('uploads/dokument.txt').read())"
```

Limit: 20 MB na plik.
