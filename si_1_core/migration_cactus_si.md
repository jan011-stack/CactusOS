# Migracja: usunięcie ról + podłączenie Cactus SI

**Data:** 2026-10-27  
**Operator:** Mateusz Mitek

## Usunięte
- Role: SI_1, Alex, Astyria, Aurelius
- Model lokalny: Cactus AI (`cactus-ai` / `cactus-ai-lite` jako główny)

## Podłączone
- **Cactus SI** jako główny backend generatywny
- Zmienna: `CACTUS_SI_API_URL` (domyślnie `https://cactus-si.ai`)
- Zmienna: `CACTUS_SI_API_KEY`

## Konfiguracja (.env)
```env
CACTUS_SI_API_URL=https://cactus-si.ai
CACTUS_SI_API_KEY=twoj_klucz
```

## Kolejność providerów
1. Cactus SI (jeśli klucz ustawiony)
2. Gemini (jeśli klucz)
3. Ollama (fallback)
4. Local NLP (ostatnia deska)

## Status
`python3 -m si_1_core --status` pokazuje teraz agenta `Cactus SI`.
