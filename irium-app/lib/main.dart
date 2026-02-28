import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

const MethodChannel _fileOpenChannel = MethodChannel('com.irium.app/file_open');

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Irium App',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.deepPurple),
      ),
      home: const MyHomePage(title: 'Irium PDF Opener'),
    );
  }
}

class MyHomePage extends StatefulWidget {
  const MyHomePage({super.key, required this.title});

  final String title;

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> {
  String? _lastOpenedPdf;

  @override
  void initState() {
    super.initState();
    _fileOpenChannel.setMethodCallHandler(_handleMethodCall);
    _syncPendingOpenFiles();
  }

  Future<void> _syncPendingOpenFiles({int attemptsLeft = 5}) async {
    try {
      final files = await _fileOpenChannel
          .invokeMethod<List<dynamic>>('flush_pending_open_files');
      if (!mounted || files == null || files.isEmpty) return;

      String? lastPath;
      for (final file in files) {
        if (file is String && file.isNotEmpty) {
          lastPath = file;
        }
      }
      if (lastPath == null) return;
      setState(() {
        _lastOpenedPdf = lastPath;
      });
    } on MissingPluginException {
      if (attemptsLeft <= 1 || !mounted) return;
      await Future<void>.delayed(const Duration(milliseconds: 250));
      if (!mounted) return;
      await _syncPendingOpenFiles(attemptsLeft: attemptsLeft - 1);
    } on PlatformException {
      // Ignore channel errors during startup; runtime open_file events still work.
    }
  }

  Future<void> _handleMethodCall(MethodCall call) async {
    if (call.method != 'open_file') return;

    final path = call.arguments as String?;
    if (path == null || !mounted) return;

    setState(() {
      _lastOpenedPdf = path;
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        title: Text(widget.title),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Text('Open With integration is active for PDF files.'),
            const SizedBox(height: 12),
            if (_lastOpenedPdf == null) ...[
              const Text('No PDF has been opened yet.')
            ] else ...[
              const Text('Last opened PDF:'),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 24),
                child: Text(
                  _lastOpenedPdf!,
                  textAlign: TextAlign.center,
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
