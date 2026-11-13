import 'package:flutter_test/flutter_test.dart';
import 'package:seal_shell/app/update_check.dart';

void main() {
  test('version comparison treats numeric components, not strings', () {
    expect(UpdateChecker.isNewer('3.0.14', '3.0.13'), isTrue);
    expect(UpdateChecker.isNewer('3.0.13', '3.0.13'), isFalse);
    expect(UpdateChecker.isNewer('3.0.9', '3.0.10'), isFalse);
    expect(UpdateChecker.isNewer('3.1.0', '3.0.99'), isTrue);
    expect(UpdateChecker.isNewer('v3.0.14', '3.0.13'), isTrue);
  });
}
