import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var pendingOpenedFiles: [String] = []
  private let openFileChannelName = "com.irium.app/file_open"
  private var openFileChannel: FlutterMethodChannel?

  override func applicationDidFinishLaunching(_ notification: Notification) {
    super.applicationDidFinishLaunching(notification)
    installOpenFileChannelIfNeeded()
  }

  override func applicationDidBecomeActive(_ notification: Notification) {
    super.applicationDidBecomeActive(notification)
    installOpenFileChannelIfNeeded()
  }

  override func application(_ application: NSApplication, open urls: [URL]) {
    for url in urls {
      notifyFlutterAboutOpenFile(url.path)
    }
  }

  override func application(_ application: NSApplication, openFile filename: String) -> Bool {
    notifyFlutterAboutOpenFile(filename)
    return true
  }

  private func notifyFlutterAboutOpenFile(_ path: String) {
    guard let extensionLower = path.split(separator: ".").last?.lowercased(),
          extensionLower == "pdf" else {
      return
    }
    sendOpenFilePathToFlutter(path)
  }

  private func sendOpenFilePathToFlutter(_ path: String) {
    installOpenFileChannelIfNeeded()
    guard let channel = openFileChannel else {
      pendingOpenedFiles.append(path)
      return
    }
    channel.invokeMethod("open_file", arguments: path)
  }

  private func installOpenFileChannelIfNeeded() {
    if openFileChannel != nil {
      return
    }

    let controller = (mainFlutterWindow?.contentViewController as? FlutterViewController)
      ?? NSApp.windows.compactMap { $0.contentViewController as? FlutterViewController }.first
    guard let controller else {
      return
    }

    openFileChannel = FlutterMethodChannel(
      name: openFileChannelName,
      binaryMessenger: controller.engine.binaryMessenger
    )
    openFileChannel?.setMethodCallHandler { [weak self] call, result in
      guard let self = self else {
        result(FlutterError(code: "unavailable", message: "App delegate unavailable", details: nil))
        return
      }
      if call.method == "flush_pending_open_files" {
        result(self.consumePendingOpenedFiles())
        return
      }
      result(FlutterMethodNotImplemented)
    }
  }

  private func consumePendingOpenedFiles() -> [String] {
    let files = pendingOpenedFiles
    pendingOpenedFiles.removeAll()
    return files
  }

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }
}
