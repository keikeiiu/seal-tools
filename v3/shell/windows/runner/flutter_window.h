#ifndef RUNNER_FLUTTER_WINDOW_H_
#define RUNNER_FLUTTER_WINDOW_H_

#include <flutter/dart_project.h>
#include <flutter/encodable_value.h>
#include <flutter/flutter_view_controller.h>
#include <flutter/method_channel.h>

#include <memory>

#include "win32_window.h"

// A window that does nothing but host a Flutter view.
class FlutterWindow : public Win32Window {
 public:
  // Creates a new FlutterWindow hosting a Flutter view running |project|.
  explicit FlutterWindow(const flutter::DartProject& project);
  virtual ~FlutterWindow();

 protected:
  // Win32Window:
  bool OnCreate() override;
  void OnDestroy() override;
  LRESULT MessageHandler(HWND window, UINT const message, WPARAM const wparam,
                         LPARAM const lparam) noexcept override;

 private:
  // The project to run.
  flutter::DartProject project_;

  // The Flutter instance hosted by this window.
  std::unique_ptr<flutter::FlutterViewController> flutter_controller_;

  // One tiny window-control channel (`seal/host`): hide/show for the capture lifecycle, the
  // always-on-top pin, and the remembered window placement. The runner is the only place the process
  // touches its own window, and, because the shell is DPI-unaware, the only place logical
  // coordinates are converted to and from physical pixels.
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> host_channel_;

  // Reports the window's current rect to Dart as `windowMoved` (logical screen coordinates): after a
  // user move/resize settles and when the window is maximized or restored. Dart debounces the
  // `ui.window` write; a no-op before the channel exists.
  void NotifyWindowMoved();
};

#endif  // RUNNER_FLUTTER_WINDOW_H_
