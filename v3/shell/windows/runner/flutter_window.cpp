#include "flutter_window.h"

#include <cmath>
#include <cstdint>
#include <optional>
#include <string>

#include <shellapi.h>
#include <variant>

#include <flutter/standard_method_codec.h>
#include <flutter_windows.h>

#include "flutter/generated_plugin_registrant.h"

namespace {

// The shell speaks LOGICAL screen coordinates (the DPI-free units v2's `ui` block stores); the OS
// works in physical pixels. The runner is the one place that converts, using the window's monitor
// DPI and rounding, the same rule Win32Window::Create uses for the initial rect - so Dart never has
// to know the scale factor.
double DpiScaleForWindow(HWND window) {
  HMONITOR monitor = ::MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
  UINT dpi = FlutterDesktopGetDpiForMonitor(monitor);
  return dpi == 0 ? 1.0 : static_cast<double>(dpi) / 96.0;
}

int LogicalToPhysical(int logical, double scale) {
  return static_cast<int>(std::lround(static_cast<double>(logical) * scale));
}

int PhysicalToLogical(int physical, double scale) {
  return static_cast<int>(std::lround(static_cast<double>(physical) / scale));
}

flutter::EncodableMap PhysicalRectToLogical(const RECT& rect, double scale) {
  flutter::EncodableMap result;
  result[flutter::EncodableValue("left")] =
      flutter::EncodableValue(PhysicalToLogical(rect.left, scale));
  result[flutter::EncodableValue("top")] =
      flutter::EncodableValue(PhysicalToLogical(rect.top, scale));
  result[flutter::EncodableValue("width")] =
      flutter::EncodableValue(PhysicalToLogical(rect.right - rect.left, scale));
  result[flutter::EncodableValue("height")] =
      flutter::EncodableValue(PhysicalToLogical(rect.bottom - rect.top, scale));
  return result;
}

// Reads one `setWindowRect` argument (the codec sends the four as numbers, which may arrive as int
// or double depending on the Dart side).
bool ReadNumber(const flutter::EncodableMap& arguments, const char* key,
                double* value) {
  auto found = arguments.find(flutter::EncodableValue(key));
  if (found == arguments.end()) {
    return false;
  }
  if (const auto* as_double = std::get_if<double>(&found->second)) {
    *value = *as_double;
    return true;
  }
  if (const auto* as_int = std::get_if<int32_t>(&found->second)) {
    *value = static_cast<double>(*as_int);
    return true;
  }
  if (const auto* as_long = std::get_if<int64_t>(&found->second)) {
    *value = static_cast<double>(*as_long);
    return true;
  }
  return false;
}

// The window's current rect in the channel's logical shape, or false when it is not usable yet (a
// zero-size window during creation).
bool CurrentLogicalRect(HWND window, double scale, flutter::EncodableMap* out) {
  RECT rect;
  if (!::GetWindowRect(window, &rect)) {
    return false;
  }
  if (rect.right - rect.left <= 0 || rect.bottom - rect.top <= 0) {
    return false;
  }
  *out = PhysicalRectToLogical(rect, scale);
  return true;
}

// The install's logs directory: next to the runner executable, never a hard-coded user path. The
// runner owns this because it is the only process that knows where the app was installed.
std::wstring InstallLogsDirectory() {
  wchar_t buffer[32768];
  const DWORD length = ::GetModuleFileNameW(nullptr, buffer, 32768);
  if (length == 0 || length >= 32768) {
    return std::wstring();
  }
  std::wstring path(buffer, length);
  const auto slash = path.find_last_of(L"\\/");
  if (slash == std::wstring::npos) {
    return std::wstring();
  }
  path.resize(slash + 1);
  return path + L"logs";
}

// Open the install logs folder, selecting the last run's file when it exists. A bare file name only:
// the shell's value is a label like pet.log, never a path this runner should trust.
bool OpenLogsFolder(const flutter::EncodableMap* arguments) {
  const std::wstring logs = InstallLogsDirectory();
  if (logs.empty()) {
    return false;
  }
  ::CreateDirectoryW(logs.c_str(), nullptr);

  std::wstring file;
  if (arguments != nullptr) {
    const auto found = arguments->find(flutter::EncodableValue("file"));
    if (found != arguments->end()) {
      if (const auto* value = std::get_if<std::string>(&found->second)) {
        file.assign(value->begin(), value->end());
      }
    }
  }
  if (file.find_first_of(L"\\/") != std::wstring::npos ||
      file.find(L"..") != std::wstring::npos) {
    file.clear();
  }

  HINSTANCE launched;
  if (!file.empty()) {
    const std::wstring target = logs + L"\\" + file;
    if (::GetFileAttributesW(target.c_str()) != INVALID_FILE_ATTRIBUTES) {
      const std::wstring parameters = L"/select,\"" + target + L"\"";
      launched = ::ShellExecuteW(nullptr, L"open", L"explorer.exe",
                                 parameters.c_str(), nullptr, SW_SHOWNORMAL);
      return reinterpret_cast<INT_PTR>(launched) > 32;
    }
  }
  launched = ::ShellExecuteW(nullptr, L"open", logs.c_str(), nullptr, nullptr,
                             SW_SHOWNORMAL);
  return reinterpret_cast<INT_PTR>(launched) > 32;
}

}  // namespace

FlutterWindow::FlutterWindow(const flutter::DartProject& project)
    : project_(project) {}

FlutterWindow::~FlutterWindow() {}

bool FlutterWindow::OnCreate() {
  if (!Win32Window::OnCreate()) {
    return false;
  }

  RECT frame = GetClientArea();

  // The size here must match the window dimensions to avoid unnecessary surface
  // creation / destruction in the startup path.
  flutter_controller_ = std::make_unique<flutter::FlutterViewController>(
      frame.right - frame.left, frame.bottom - frame.top, project_);
  // Ensure that basic setup of the controller was successful.
  if (!flutter_controller_->engine() || !flutter_controller_->view()) {
    return false;
  }
  RegisterPlugins(flutter_controller_->engine());
  SetChildContent(flutter_controller_->view()->GetNativeWindow());

  // The capture lifecycle: hide, wait a frame, capture, then show. The hide lives here because the
  // runner is the only place that owns the HWND; the Dart side awaits each reply, so the window is
  // really hidden before the core checks the foreground window (`game_not_foreground` otherwise).
  // `setAlwaysOnTop` is the persisted pin: applied once on load and on toggle, never per frame, so
  // it cannot interfere with the capture's hide/show. The window placement methods are the other
  // half of v2's remembered geometry: `windowRect`/`primaryWorkArea` report and `setWindowRect`
  // applies logical coordinates, converted here (`windowMoved` reports the resting rect back).
  host_channel_ =
      std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          flutter_controller_->engine()->messenger(), "seal/host",
          &flutter::StandardMethodCodec::GetInstance());
  host_channel_->SetMethodCallHandler(
      [this](const flutter::MethodCall<flutter::EncodableValue>& call,
             std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>>
                 result) {
        if (call.method_name() == "hideWindow") {
          ::ShowWindow(GetHandle(), SW_HIDE);
          result->Success();
        } else if (call.method_name() == "showWindow") {
          ::ShowWindow(GetHandle(), SW_SHOWNORMAL);
          result->Success();
        } else if (call.method_name() == "setAlwaysOnTop") {
          const auto* enabled = std::get_if<bool>(call.arguments());
          if (enabled == nullptr) {
            result->Error("bad_arguments",
                          "setAlwaysOnTop expects a bool");
            return;
          }
          ::SetWindowPos(GetHandle(),
                         *enabled ? HWND_TOPMOST : HWND_NOTOPMOST, 0, 0, 0, 0,
                         SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
          result->Success();
        } else if (call.method_name() == "windowRect") {
          const double scale = DpiScaleForWindow(GetHandle());
          flutter::EncodableMap rect;
          if (!CurrentLogicalRect(GetHandle(), scale, &rect)) {
            result->Error("bad_state", "the window has no rect yet");
            return;
          }
          result->Success(flutter::EncodableValue(rect));
        } else if (call.method_name() == "primaryWorkArea") {
          MONITORINFO info{};
          info.cbSize = static_cast<DWORD>(sizeof(MONITORINFO));
          HMONITOR monitor =
              ::MonitorFromWindow(GetHandle(), MONITOR_DEFAULTTOPRIMARY);
          if (!::GetMonitorInfo(monitor, &info)) {
            result->Error("bad_state", "the monitor has no work area");
            return;
          }
          result->Success(flutter::EncodableValue(
              PhysicalRectToLogical(info.rcWork,
                                    DpiScaleForWindow(GetHandle()))));
        } else if (call.method_name() == "virtualScreen") {
          // Every monitor's bounding rect: a saved placement outside it was on a monitor that is
          // gone. The metrics are in physical pixels for a DPI-aware process; the window's monitor
          // scale converts them like every other rect (mixed-DPI desktops are approximate here,
          // the clamp is a safety net, not a layout tool).
          RECT rect;
          rect.left = ::GetSystemMetrics(SM_XVIRTUALSCREEN);
          rect.top = ::GetSystemMetrics(SM_YVIRTUALSCREEN);
          rect.right = rect.left + ::GetSystemMetrics(SM_CXVIRTUALSCREEN);
          rect.bottom = rect.top + ::GetSystemMetrics(SM_CYVIRTUALSCREEN);
          result->Success(flutter::EncodableValue(PhysicalRectToLogical(
              rect, DpiScaleForWindow(GetHandle()))));
        } else if (call.method_name() == "setWindowMinSize") {
          // The mini running card needs a window smaller than the full shell's 1120x680 minimum.
          // Dart lowers this before resizing and restores it when the run ends.
          const auto* arguments =
              std::get_if<flutter::EncodableMap>(call.arguments());
          double width = 0;
          double height = 0;
          if (arguments == nullptr ||
              !ReadNumber(*arguments, "width", &width) ||
              !ReadNumber(*arguments, "height", &height)) {
            result->Error("bad_arguments",
                          "setWindowMinSize expects width/height");
            return;
          }
          SetMinimumSize(width, height);
          result->Success();
        } else if (call.method_name() == "setMiniChrome") {
          // Borderless, non-activating mini running card; `false` restores the full shell's chrome.
          const auto* mini = std::get_if<bool>(call.arguments());
          if (mini == nullptr) {
            result->Error("bad_arguments", "setMiniChrome expects a bool");
            return;
          }
          SetMiniChrome(*mini);
          result->Success();
        } else if (call.method_name() == "setWindowRect") {
          const auto* arguments =
              std::get_if<flutter::EncodableMap>(call.arguments());
          double left = 0;
          double top = 0;
          double width = 0;
          double height = 0;
          if (arguments == nullptr ||
              !ReadNumber(*arguments, "left", &left) ||
              !ReadNumber(*arguments, "top", &top) ||
              !ReadNumber(*arguments, "width", &width) ||
              !ReadNumber(*arguments, "height", &height)) {
            result->Error("bad_arguments",
                          "setWindowRect expects left/top/width/height");
            return;
          }
          const double scale = DpiScaleForWindow(GetHandle());
          ::SetWindowPos(
              GetHandle(), nullptr,
              LogicalToPhysical(static_cast<int>(std::lround(left)), scale),
              LogicalToPhysical(static_cast<int>(std::lround(top)), scale),
              LogicalToPhysical(static_cast<int>(std::lround(width)), scale),
              LogicalToPhysical(static_cast<int>(std::lround(height)), scale),
              SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
          result->Success();
        } else if (call.method_name() == "isWindowVisible") {
          result->Success(
              flutter::EncodableValue(::IsWindowVisible(GetHandle()) != 0));
        } else if (call.method_name() == "openLogsFolder") {
          const auto* arguments =
              std::get_if<flutter::EncodableMap>(call.arguments());
          if (!OpenLogsFolder(arguments)) {
            result->Error("host_unavailable",
                          "could not open the logs folder");
            return;
          }
          result->Success();
        } else {
          result->NotImplemented();
        }
      });

  flutter_controller_->engine()->SetNextFrameCallback([&]() {
    this->Show();
  });

  // Flutter can complete the first frame before the "show window" callback is
  // registered. The following call ensures a frame is pending to ensure the
  // window is shown. It is a no-op if the first frame hasn't completed yet.
  flutter_controller_->ForceRedraw();

  return true;
}

void FlutterWindow::OnDestroy() {
  host_channel_ = nullptr;
  if (flutter_controller_) {
    flutter_controller_ = nullptr;
  }

  Win32Window::OnDestroy();
}

LRESULT
FlutterWindow::MessageHandler(HWND hwnd, UINT const message,
                              WPARAM const wparam,
                              LPARAM const lparam) noexcept {
  // The remembered placement: report the rect when a user move/resize SETTLES (`WM_EXITSIZEMOVE`)
  // and on maximize/restore. This runs before Flutter's top-level handler, which claims `WM_SIZE`.
  // Dart debounces the write, so a handful of reports is fine; there is no per-frame work here.
  if (message == WM_EXITSIZEMOVE) {
    NotifyWindowMoved();
  } else if (message == WM_SIZE &&
             (wparam == SIZE_MAXIMIZED || wparam == SIZE_RESTORED)) {
    NotifyWindowMoved();
  }

  // Give Flutter, including plugins, an opportunity to handle window messages.
  if (flutter_controller_) {
    std::optional<LRESULT> result =
        flutter_controller_->HandleTopLevelWindowProc(hwnd, message, wparam,
                                                      lparam);
    if (result) {
      return *result;
    }
  }

  switch (message) {
    case WM_FONTCHANGE:
      flutter_controller_->engine()->ReloadSystemFonts();
      break;
  }

  return Win32Window::MessageHandler(hwnd, message, wparam, lparam);
}

void FlutterWindow::NotifyWindowMoved() {
  // `OnCreate` installs the channel; anything earlier (the window being created, or a size message
  // during `SetWindowPos` from `setWindowRect` before the engine is up) has nothing to report to.
  if (!host_channel_ || !flutter_controller_) {
    return;
  }
  flutter::EncodableMap rect;
  if (!CurrentLogicalRect(GetHandle(), DpiScaleForWindow(GetHandle()), &rect)) {
    return;
  }
  host_channel_->InvokeMethod(
      "windowMoved", std::make_unique<flutter::EncodableValue>(rect));
}
