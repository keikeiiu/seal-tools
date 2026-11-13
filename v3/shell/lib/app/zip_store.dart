import 'dart:convert';
import 'dart:typed_data';

/// A tiny ZIP writer for the diagnostics export.
///
/// The app only needs a handful of small text logs, so adding a compression dependency just for this
/// would make the shell depend on a package the rest of the UI never uses. These entries are stored
/// (method 0) and are still a standard `.zip`; Windows Explorer, 7-Zip and macOS open it normally.
class ZipStoreBuilder {
  final List<_StoredEntry> _entries = [];

  void addFile(String name, List<int> bytes) {
    _entries.add(_StoredEntry(name, bytes));
  }

  Uint8List build() {
    final local = BytesBuilder(copy: false);
    final central = BytesBuilder(copy: false);
    final now = DateTime.now();
    final dosTime = _dosTime(now);
    final dosDate = _dosDate(now);

    for (final entry in _entries) {
      final name = utf8.encode(entry.name);
      final data = Uint8List.fromList(entry.bytes);
      final crc = _crc32(data);
      final offset = local.length;

      local
        ..add(_u32(0x04034b50))
        ..add(_u16(20)) // version needed
        ..add(_u16(0x0800)) // UTF-8 file name
        ..add(_u16(0)) // store
        ..add(_u16(dosTime))
        ..add(_u16(dosDate))
        ..add(_u32(crc))
        ..add(_u32(data.length))
        ..add(_u32(data.length))
        ..add(_u16(name.length))
        ..add(_u16(0)) // extra length
        ..add(name)
        ..add(data);

      central
        ..add(_u32(0x02014b50))
        ..add(_u16(20)) // version made by
        ..add(_u16(20)) // version needed
        ..add(_u16(0x0800))
        ..add(_u16(0))
        ..add(_u16(dosTime))
        ..add(_u16(dosDate))
        ..add(_u32(crc))
        ..add(_u32(data.length))
        ..add(_u32(data.length))
        ..add(_u16(name.length))
        ..add(_u16(0)) // extra
        ..add(_u16(0)) // comment
        ..add(_u16(0)) // disk
        ..add(_u16(0)) // internal attributes
        ..add(_u32(0)) // external attributes
        ..add(_u32(offset))
        ..add(name);
    }

    final centralBytes = central.takeBytes();
    final localBytes = local.takeBytes();
    final out = BytesBuilder(copy: false)
      ..add(localBytes)
      ..add(centralBytes)
      ..add(_u32(0x06054b50))
      ..add(_u16(0)) // disk number
      ..add(_u16(0)) // central directory disk
      ..add(_u16(_entries.length))
      ..add(_u16(_entries.length))
      ..add(_u32(centralBytes.length))
      ..add(_u32(localBytes.length))
      ..add(_u16(0)); // comment
    return out.takeBytes();
  }

  static int _dosTime(DateTime time) =>
      (time.hour << 11) | (time.minute << 5) | (time.second ~/ 2);

  static int _dosDate(DateTime time) =>
      ((time.year - 1980).clamp(0, 127) << 9) | (time.month << 5) | time.day;

  static Uint8List _u16(int value) =>
      Uint8List(2)..buffer.asByteData().setUint16(0, value, Endian.little);

  static Uint8List _u32(int value) =>
      Uint8List(4)..buffer.asByteData().setUint32(0, value, Endian.little);

  static int _crc32(List<int> data) {
    var crc = 0xFFFFFFFF;
    for (final byte in data) {
      crc ^= byte;
      for (var bit = 0; bit < 8; bit++) {
        crc = (crc & 1) == 1 ? (crc >> 1) ^ 0xEDB88320 : crc >> 1;
      }
    }
    return crc ^ 0xFFFFFFFF;
  }
}

class _StoredEntry {
  const _StoredEntry(this.name, this.bytes);

  final String name;
  final List<int> bytes;
}
