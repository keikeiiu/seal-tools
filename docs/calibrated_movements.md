# Gem Composer Calibrated D Movements
All values in Arduino D units. Tested on 4K 150% DPI.

## Grade Selection (SetCursorPos, then click to focus+select)
| Grade | Position (scaled) | Physical |
|-------|-------------------|----------|
| N | (727, 696) | (1091, 1044) |
| G | (777, 696) | (1165, 1044) |
| DG | (827, 696) | (1241, 1044) |

## Radio to Register
| From | D Movement |
|------|-----------|
| N → Register | D 65 20 |
| G → Register | D 35 20 |
| DG → Register | D 10 20 |

## Register ↔ Combine
| Direction | D Movement |
|-----------|-----------|
| Register → Combine | D -30 80 |
| Combine → Register | D 30 -80 |

## Grade to Grade (radio row)
| Direction | D Movement |
|-----------|-----------|
| N → G | D 33 0 |
| G → DG | D 33 0 |
| DG → G | D -33 0 |

## Register → Slot Cleanup (right-click to remove gems)
| Direction | D Movement | Action |
|-----------|-----------|--------|
| Register → Slot1 | D -45 -100 | R (right click) |
| Slot1 → Slot2 | D 36 0 | R (right click) |

## Slot2 → Grade Radio
| Direction | D Movement |
|-----------|-----------|
| Slot2 → DG | D 8 87 |
| Slot2 → G | D 8 87 then D -33 0 |
