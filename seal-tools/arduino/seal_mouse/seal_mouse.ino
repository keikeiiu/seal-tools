#include <Mouse.h>
#include <Keyboard.h>

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
    if (Serial.available() > 0) {
        String cmd = Serial.readStringUntil('\n');
        cmd.trim();
        if (cmd.length() == 0) return;

        char type = cmd[0];

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
            int sp1 = cmd.indexOf(' ', 2);
            if (sp1 > 0) {
                int dx = cmd.substring(2, sp1).toInt();
                int dy = cmd.substring(sp1 + 1).toInt();
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
            int sp1 = cmd.indexOf(' ', 2);
            int sp2 = cmd.indexOf(' ', sp1 + 1);
            if (sp1 > 0 && sp2 > 0) {
                int dx = cmd.substring(2, sp1).toInt();
                int dy = cmd.substring(sp1 + 1, sp2).toInt();
                int dur = cmd.substring(sp2 + 1).toInt();
                if (dur < 20) dur = 20;
                if (dur > 5000) dur = 5000;
                humanMove(dx, dy, dur);
            }
        }
        // Keyboard
        else if (type == 'E') {
            Keyboard.press(KEY_RETURN);
            delay(random(30, 80));
            Keyboard.release(KEY_RETURN);
        }
        else if (type == 'T') {
            Keyboard.press(KEY_TAB);
            delay(random(30, 80));
            Keyboard.release(KEY_TAB);
        }
        else if (type == 'S') {
            Keyboard.press(' ');
            delay(random(30, 80));
            Keyboard.release(' ');
        }
        else if (type == 'K') {
            int n = cmd.substring(1).toInt();
            char key = '0' + (n % 10);
            Keyboard.press(key);
            delay(random(30, 80));
            Keyboard.release(key);
        }
        else if (type == 'F') {
            int n = cmd.substring(1).toInt();
            uint8_t fkeys[] = {0, KEY_F1, KEY_F2, KEY_F3, KEY_F4, KEY_F5,
                               KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_F10};
            if (n >= 1 && n <= 10) {
                Keyboard.press(fkeys[n]);
                delay(random(30, 80));
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
            int ms = cmd.substring(2).toInt();
            if (ms > 0 && ms < 10000) delay(ms);
        }
    }
}
