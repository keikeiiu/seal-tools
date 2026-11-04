/// The shared grade-ladder component: fixed path, role markers and existing element keys.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/components/grade_ladder.dart';
import 'package:seal_shell/ui/theme.dart';

void main() {
  testWidgets('marks the target, minimum and start roles in place', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: buildShellTheme(),
        home: const Scaffold(
          body: GradeLadder(
            grades: ['N', 'G', 'DG', 'XG', 'SG'],
            keyPrefix: 'grade-order',
            target: 'G',
            minimum: 'DG',
          ),
        ),
      ),
    );

    expect(find.text('target'), findsOneWidget);
    expect(find.text('minimum'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('grade-order-target-legend')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('grade-order-minimum-legend')),
      findsOneWidget,
    );
    for (final grade in ['N', 'G', 'DG', 'XG', 'SG']) {
      expect(find.byKey(ValueKey('grade-order-$grade')), findsOneWidget);
    }
  });

  testWidgets('marks the composer start and no target', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        theme: buildShellTheme(),
        home: const Scaffold(
          body: GradeLadder(
            grades: ['N', 'G', 'DG'],
            keyPrefix: 'gem-grade',
            start: 'G',
          ),
        ),
      ),
    );

    expect(find.text('start'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('gem-grade-start-legend')),
      findsOneWidget,
    );
    expect(find.text('target'), findsNothing);
  });
}
