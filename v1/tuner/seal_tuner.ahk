#Requires AutoHotkey v2.0
Persistent
#SingleInstance Force

MButton:: {
    Static on := False
    If on := !on
        SetTimer(spam, 500), SoundBeep(1500)
    Else
        SetTimer(spam, 0), SoundBeep(1000)
}

spam() {
    SendEvent "{Enter}"
}
