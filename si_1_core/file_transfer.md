# Terminal — załączniki i generowanie obrazów

Uruchomienie:
```bash
bash scripts/file_transfer.sh
```
Adres: http://127.0.0.1:8765/

## Wysyłanie plików
```bash
curl -F 'file=@dokument.txt' http://127.0.0.1:8765/upload
```

## Lista plików
```bash
curl http://127.0.0.1:8765/files
```

## Pobieranie (terminal wysyła załącznik)
```bash
curl -O http://127.0.0.1:8765/download/dokument.txt
```

## Generowanie obrazu
```bash
curl -X POST http://127.0.0.1:8765/generate-image \
  -H 'Content-Type: application/json' \
  -d '{"prompt":"zielony kaktus na pustyni"}'
```

Domyślnie generuje SVG (fallback).  
Aby użyć zewnętrznego API obrazów, ustaw w `.env`:
```env
CACTUS_IMAGE_API_URL=https://twoje-api/generate
CACTUS_IMAGE_API_KEY=opcjonalny_klucz
```

Wygenerowane pliki lądują w `uploads/generated/`.
