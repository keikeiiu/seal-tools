#include <Mouse.h>
#include <Keyboard.h>
#include <stdio.h>
#include <stdlib.h>

// ── Serial protocol ──────────────────────────
// One command per line, '\n'-terminated, 115200 baud. The first character selects the command.
//
//   C            left click (hold 50-150 ms)
//   R            right click
//   D dx dy      relative move — RAW HID counts, walked out in 10-px chunks. Never DPI-scaled.
//   H dx dy ms   Bezier human-like move (v2 does not use it)
//   E            Enter
//   T            Tab
//   S            Space (tap: press, hold, release)
//   K c / k c    one printable character (letter or digit), normal / fast hold
//   F n / f n    F1-F12, normal / fast hold
//   X            Alt+Tab
//   W ms         wait ms (1-9999)
//   Q n / Z n    mouse wheel up / down, n notches (1-200)
//   P / U        hold / release the spacebar (auto-pickup)
//
// An unrecognised letter is ignored, and so is a K/F/Q/Z frame whose argument is out of range —
// nothing is guessed at. The launcher validates before sending; this is the second line of defence.

// ── Key hold durations (ms) ──────────────────
// Uppercase K/F = human-like hold (30-80ms), lowercase k/f = fast hold.
#define HOLD_NORMAL_MIN  30
#define HOLD_NORMAL_MAX  80
#define HOLD_FAST        10

// ── Mouse wheel ──────────────────────────────
// Games read the wheel as DISCRETE notches, so a scroll of n is n separate Mouse.move events with a
// gap between them — one Mouse.move(0, 0, n) usually registers as a single notch. The gap is what
// makes a multi-notch scroll land; too fast and the game coalesces or drops them.
#define WHEEL_NOTCH_GAP_MS 25
// Ceiling for a single scroll command, and — because "scroll to the top" is sent as ONE command at
// this maximum — also how far the tool can scroll in a single go. A list longer than this leaves the
// origin short of the top, and every stored scroll amount is measured from that origin, so the value
// is load-bearing rather than just a guard. 200 notches at the gap below is about five seconds.
// Lower it only if the guard matters more than the reach.
#define WHEEL_MAX_NOTCHES  200

// ── Held-key failsafe ────────────────────────
// 'P' presses the spacebar and leaves it held until 'U'. If the host disappears while it is held
// — the launcher crashed or was killed — nothing would ever send that 'U' and the key would stay
// down on the host until the device is unplugged. `Serial` is false once the host closes the port
// (the same signal the usual `while (!Serial)` wait uses), so the loop releases everything when
// that happens. The app holds the port open for its whole lifetime, so this only fires when it is
// genuinely gone.
static bool spaceHeld = false;

// ── Bezier human-like mouse movement ─────────
float cubicBezier(float p0, float p1, float p2, float p3, float t) {
    float u = 1.0f - t;
    return (u*u*u*p0) + (3*u*u*t*p1) + (3*u*t*t*p2) + (t*t*t*p3);
}

void humanMove(int targetX, int targetY, int durationMs) {
    if (durationMs < 20) durationMs = 20;

    // Fixed control points for PREDICTABLE movement (no random jitter in path)
    float cX1 = targetX * 0.30f;
    float cY1 = targetY * 0.15f;
    float cX2 = targetX * 0.70f;
    float cY2 = targetY * 0.85f;

    int steps = durationMs / 5;
    if (steps < 4) steps = 4;

    float lastX = 0, lastY = 0;
    for (int i = 1; i <= steps; i++) {
        float progress = (float)i / steps;
        float t = (1.0f - cos(progress * PI)) / 2.0f;  // ease-in-out

        float curX = cubicBezier(0, cX1, cX2, targetX, t);
        float curY = cubicBezier(0, cY1, cY2, targetY, t);

        int dx = (int)(curX - lastX);
        int dy = (int)(curY - lastY);

        // Micro-jitter: 10% chance
        if (random(0, 100) < 10) {
            dx += random(-1, 2);
            dy += random(-1, 2);
        }

        if (dx != 0 || dy != 0) {
            Mouse.move(dx, dy, 0);
        }
        lastX = curX; lastY = curY;
        delay(random(4, 7));
    }
}

// ── Setup ────────────────────────────────────
void setup() {
    Serial.begin(115200);
    delay(3000);
    Mouse.begin();
    Keyboard.begin();
    randomSeed(analogRead(0));
}

// ── Loop ─────────────────────────────────────
void loop() {
    static char buf[32];  // fixed buffer — no heap allocation (avoids String fragmentation)

    // Host gone while a key is being held: release it rather than leave it stuck down.
    if (!Serial && spaceHeld) {
        Keyboard.releaseAll();
        spaceHeld = false;
    }

    if (Serial.available() > 0) {
        size_t n = Serial.readBytesUntil('\n', buf, sizeof(buf) - 1);
        buf[n] = '\0';
        if (n == 0) return;

        char type = buf[0];

        // Mouse click
        if (type == 'C') {
            Mouse.press(MOUSE_LEFT);
            delay(random(50, 150));
            Mouse.release(MOUSE_LEFT);
        }
        else if (type == 'R') {
            Mouse.press(MOUSE_RIGHT);
            delay(random(50, 150));
            Mouse.release(MOUSE_RIGHT);
        }
        // Direct move: "D dx dy" — no curves, straight line
        else if (type == 'D') {
            int dx = 0, dy = 0;
            if (sscanf(buf, "D %d %d", &dx, &dy) == 2) {
                int sx = (dx > 0) ? 1 : -1;
                int sy = (dy > 0) ? 1 : -1;
                int ax = abs(dx), ay = abs(dy);
                while (ax > 0 || ay > 0) {
                    int sx2 = (ax > 10) ? 10 : ax;
                    int sy2 = (ay > 10) ? 10 : ay;
                    if (sx2 == 0 && sy2 == 0) break;
                    Mouse.move(sx * sx2, sy * sy2);
                    ax -= sx2; ay -= sy2;
                    delay(1);
                }
            }
        }
        // Human-like move: "H dx dy duration"
        else if (type == 'H') {
            int dx = 0, dy = 0, dur = 0;
            if (sscanf(buf, "H %d %d %d", &dx, &dy, &dur) == 3) {
                if (dur < 20) dur = 20;
                if (dur > 5000) dur = 5000;
                humanMove(dx, dy, dur);
            }
        }
        // Keyboard — normal (human-like) hold
        else if (type == 'E') {
            Keyboard.press(KEY_RETURN);
            delay(random(HOLD_NORMAL_MIN, HOLD_NORMAL_MAX));
            Keyboard.release(KEY_RETURN);
        }
        else if (type == 'T') {
            Keyboard.press(KEY_TAB);
            delay(random(HOLD_NORMAL_MIN, HOLD_NORMAL_MAX));
            Keyboard.release(KEY_TAB);
        }
        else if (type == 'S') {
            Keyboard.press(' ');
            delay(random(HOLD_NORMAL_MIN, HOLD_NORMAL_MAX));
            Keyboard.release(' ');
        }
        // K/F = normal hold, k/f = fast hold
        // "K c" / "k c" — press ONE printable character, normal / fast hold. c is any byte in
        // 0x20-0x7E, so letters work as well as digits. This replaces the old digit index, and is
        // behaviour-compatible with it: "K 5" presses '5' exactly as before, because '0' + 5 was
        // also '5'. Both "K c" and "Kc" parse, so a space separator is optional.
        else if (type == 'K' || type == 'k') {
            char key = (buf[1] == ' ') ? buf[2] : buf[1];
            if (key >= 0x20 && key <= 0x7E) {
                int hold = (type == 'k') ? HOLD_FAST : random(HOLD_NORMAL_MIN, HOLD_NORMAL_MAX);
                Keyboard.press(key);
                delay(hold);
                Keyboard.release(key);
            }
        }
        else if (type == 'F' || type == 'f') {
            int n = atoi(&buf[1]);
            uint8_t fkeys[] = {0, KEY_F1, KEY_F2, KEY_F3, KEY_F4, KEY_F5,
                               KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_F10, KEY_F11, KEY_F12};
            if (n >= 1 && n <= 12) {
                int hold = (type == 'f') ? HOLD_FAST : random(HOLD_NORMAL_MIN, HOLD_NORMAL_MAX);
                Keyboard.press(fkeys[n]);
                delay(hold);
                Keyboard.release(fkeys[n]);
            }
        }
        // Alt+Tab
        else if (type == 'X') {
            Keyboard.press(KEY_LEFT_ALT);
            delay(30); Keyboard.press(KEY_TAB);
            delay(50); Keyboard.release(KEY_TAB);
            delay(30); Keyboard.release(KEY_LEFT_ALT);
        }
        // Wait
        else if (type == 'W') {
            int ms = atoi(&buf[1]);
            if (ms > 0 && ms < 10000) delay(ms);
        }
        // Mouse wheel — "Q n" scrolls up n notches, "Z n" scrolls down n. One Mouse.move per notch,
        // with a gap, because games count discrete wheel events (see the defines above).
        else if (type == 'Q' || type == 'Z') {
            int n = atoi(&buf[1]);
            if (n < 1) n = 1;
            if (n > WHEEL_MAX_NOTCHES) n = WHEEL_MAX_NOTCHES;
            int notch = (type == 'Q') ? 1 : -1;
            for (int i = 0; i < n; i++) {
                Mouse.move(0, 0, notch);
                delay(WHEEL_NOTCH_GAP_MS);
            }
        }
        // Hold space (auto-pickup): P presses and keeps it held, U releases it.
        else if (type == 'P') {
            Keyboard.press(' ');
            spaceHeld = true;
        }
        else if (type == 'U') {
            Keyboard.release(' ');
            spaceHeld = false;
        }
    }
}
