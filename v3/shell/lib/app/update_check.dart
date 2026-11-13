import 'dart:convert';
import 'dart:io';

/// The latest published release, as far as the shell needs it.
class UpdateInfo {
  const UpdateInfo({
    required this.version,
    required this.releaseUrl,
    required this.notes,
  });

  final String version;
  final String releaseUrl;
  final String notes;
}

/// Checks the public GitHub release feed for a newer Seal Tools version.
///
/// This is intentionally CHECK-ONLY: it never downloads or replaces an executable. The update path
/// stays the signed/checksummed installer or the portable zip the player downloads themselves.
class UpdateChecker {
  UpdateChecker({HttpClient? client}) : _client = client ?? HttpClient();

  final HttpClient _client;
  static final Uri _latestUri = Uri.parse(
    'https://api.github.com/repos/keikeiiu/seal-tools/releases/latest',
  );

  Future<UpdateInfo?> latest({String currentVersion = ''}) async {
    _client.connectionTimeout = const Duration(seconds: 10);
    final request = await _client.getUrl(_latestUri);
    request.headers
      ..set(HttpHeaders.userAgentHeader, 'SealTools/$currentVersion')
      ..set(HttpHeaders.acceptHeader, 'application/vnd.github+json');
    final response = await request.close().timeout(const Duration(seconds: 10));
    if (response.statusCode != HttpStatus.ok) {
      throw HttpException('GitHub returned HTTP ${response.statusCode}');
    }
    final body = await utf8.decoder.bind(response).join();
    final decoded = jsonDecode(body);
    if (decoded is! Map<String, dynamic>) {
      throw const FormatException('GitHub release response is not an object');
    }
    final tag = decoded['tag_name'];
    final url = decoded['html_url'];
    final notes = decoded['body'];
    if (tag is! String || url is! String) {
      throw const FormatException(
        'GitHub release response is missing tag_name/html_url',
      );
    }
    final version = tag.startsWith('v') ? tag.substring(1) : tag;
    return UpdateInfo(
      version: version,
      releaseUrl: url,
      notes: notes is String ? notes : '',
    );
  }

  /// Whether [candidate] is newer than [current], comparing numeric x.y.z components.
  static bool isNewer(String candidate, String current) {
    final next = _parts(candidate);
    final now = _parts(current);
    final length = next.length > now.length ? next.length : now.length;
    for (var index = 0; index < length; index++) {
      final a = index < next.length ? next[index] : 0;
      final b = index < now.length ? now[index] : 0;
      if (a != b) {
        return a > b;
      }
    }
    return false;
  }

  static List<int> _parts(String version) {
    final normalized = version.startsWith('v') ? version.substring(1) : version;
    final core = normalized.split('-').first;
    return [for (final part in core.split('.')) int.tryParse(part) ?? 0];
  }
}
