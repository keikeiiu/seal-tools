# Gem Composer Automation Flow

## UI Reference
See `coordinates_v3.png` for button positions on the gem compose UI.

## Calibrated Movements
All values in Arduino D units. See `calibrated_movements.md` for full reference.

## Normal Combine Flow (per grade)

```
START: Grade radio selected (SetCursorPos + Click)
  │
  ├─ 1. D_TO_REGISTER[grade] → Click Register
  │     N:  D 65 20
  │     G:  D 35 20
  │     DG: D 10 20
  │
  └─ 2. LOOP:
        ├─ D -30 80  → Click Combine
        ├─ D 30 -80  → Click Register (deregister)
        └─ Click Register again (register new set)
```

## Depletion Cleanup Flow
When a grade runs out (< 3 gems remain, combine won't work):
```
FROM Register (after last failed attempt):
  │
  ├─ 1. D -45 -100 → Right-click Slot1 (remove gem)
  ├─ 2. D 36 0    → Right-click Slot2 (remove gem)
  │
  └─ 3. Move to next grade:
        ├─ To G:  already at G (first grade)
        ├─ To DG: D 8 87 → Click DG
        └─ Complete: D 8 87 → D -33 0 → Click G (for N→G advancement)
```

## Full N → G → DG Sequence
```
1. Select N radio (SetCursorPos 727,696) → Click
2. D 65 20 → Click Register
3. LOOP: Combine → Register×2 (repeat until depleted)
4. Cleanup: Slot1(R) → Slot2(R) → DG(D 8 87 click)
5. D 10 20 → Click Register (DG→Register is short)
6. LOOP: Combine → Register×2
7. Cleanup: Slot1(R) → Slot2(R) → G(D 8 87 + D -33 0 click)
8. D 35 20 → Click Register (G→Register)
9. LOOP: Combine → Register×2
[DONE]

Note: N goes first, then DG (skip G for now since N directly creates DG-grade gems).
To include G: swap steps 3-4 above.
```

## Arduino Commands Reference
| Command | Action |
|---------|--------|
| `C` | Left click |
| `R` | Right click |
| `D dx dy` | Direct mouse move (no curve) |
| `H dx dy dur` | Human-like Bezier move |
| `E` | Enter key |
| `F n` | F1-F10 |
| `K n` | Number key 0-9 |
| `T` | Tab |
| `S` | Space |
| `X` | Alt+Tab |
