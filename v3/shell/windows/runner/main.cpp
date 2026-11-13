#include <flutter/dart_project.h>
#include <flutter/flutter_view_controller.h>
#include <windows.h>

#include "flutter_window.h"
#include "utils.h"

namespace {

constexpr wchar_t kAppTitle[] = L"Seal Tools v3";

// One app per Windows session. The handle is held for the process lifetime; the kernel releases
// it on exit, so a crash cannot leave a stale lock behind. The name is Local\ because a second
// logged-in user is a different player and must be able to run their own copy.
constexpr wchar_t kSingleInstanceMutex[] =
    L"Local\\SealTools-v3-single-instance";

// Bring an already-running launcher forward instead of opening a second window. A second launch is a
// common accident (desktop shortcut + pinned taskbar); exiting silently is worse than focusing the
// instance the player already has.
void FocusRunningWindow() {
  // The first instance may still be creating its window when the second one starts. Poll briefly
  // rather than failing the focus attempt immediately.
  for (int attempt = 0; attempt < 20; ++attempt) {
    HWND existing = ::FindWindowW(nullptr, kAppTitle);
    if (existing != nullptr) {
      if (::IsIconic(existing)) {
        ::ShowWindow(existing, SW_RESTORE);
      }
      ::SetForegroundWindow(existing);
      return;
    }
    ::Sleep(50);
  }
}

}  // namespace

int APIENTRY wWinMain(_In_ HINSTANCE instance, _In_opt_ HINSTANCE prev,
                      _In_ wchar_t *command_line, _In_ int show_command) {
  // Attach to console when present (e.g., 'flutter run') or create a
  // new console when running with a debugger.
  if (!::AttachConsole(ATTACH_PARENT_PROCESS) && ::IsDebuggerPresent()) {
    CreateAndAttachConsole();
  }

  // Initialize COM, so that it is available for use in the library and/or
  // plugins.
  ::CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

  HANDLE single_instance =
      ::CreateMutexW(nullptr, FALSE, kSingleInstanceMutex);
  if (single_instance == nullptr) {
    return EXIT_FAILURE;
  }
  if (::GetLastError() == ERROR_ALREADY_EXISTS) {
    FocusRunningWindow();
    ::CloseHandle(single_instance);
    ::CoUninitialize();
    return EXIT_SUCCESS;
  }

  flutter::DartProject project(L"data");

  std::vector<std::string> command_line_arguments =
      GetCommandLineArguments();

  project.set_dart_entrypoint_arguments(std::move(command_line_arguments));

  FlutterWindow window(project);
  Win32Window::Point origin(10, 10);
  Win32Window::Size size(1280, 720);
  if (!window.Create(kAppTitle, origin, size)) {
    ::CloseHandle(single_instance);
    ::CoUninitialize();
    return EXIT_FAILURE;
  }
  window.SetQuitOnClose(true);

  ::MSG msg;
  while (::GetMessage(&msg, nullptr, 0, 0)) {
    ::TranslateMessage(&msg);
    ::DispatchMessage(&msg);
  }

  ::CloseHandle(single_instance);
  ::CoUninitialize();
  return EXIT_SUCCESS;
}
