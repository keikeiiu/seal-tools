/// [ShellRail] — the 216 px navigation rail (RUN / TOOLS / SYSTEM), replacing the 12 flat tabs
/// (D1 of [PLAN-V3-UI-UX.md] §0/§3).
///
/// The Rail is presentation: it reports the selected destination and the running-tool count it is
/// given; selecting is the page's decision. Hold Space has no row (D2: nothing to configure) but the
/// Cards badge counts it as running like any other tool.
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';

/// One navigable destination. The first entry is the Cards screen; the rest map 1:1 to a config tab.
class ShellDestination {
  const ShellDestination({
    required this.key,
    required this.label,
    required this.icon,
    required this.group,
  });

  final String key;
  final String label;
  final IconData icon;
  final String group;
}

/// The rail order is the plan's: Run, then Tools, then System. Index == content index.
const List<ShellDestination> shellDestinations = [
  ShellDestination(
    key: 'cards',
    label: 'Cards',
    icon: Icons.dashboard_outlined,
    group: 'RUN',
  ),
  ShellDestination(
    key: 'spammer',
    label: 'Skill Spammer',
    icon: Icons.bolt_outlined,
    group: 'TOOLS',
  ),
  ShellDestination(
    key: 'tuner',
    label: 'Magic Tuner',
    icon: Icons.tune,
    group: 'TOOLS',
  ),
  ShellDestination(
    key: 'gem',
    label: 'Gem Composer',
    icon: Icons.diamond_outlined,
    group: 'TOOLS',
  ),
  ShellDestination(
    key: 'buy',
    label: 'Buy',
    icon: Icons.shopping_cart_outlined,
    group: 'TOOLS',
  ),
  ShellDestination(
    key: 'sell',
    label: 'Sell',
    icon: Icons.sell_outlined,
    group: 'TOOLS',
  ),
  ShellDestination(
    key: 'quest',
    label: 'Quest Hand-in',
    icon: Icons.assignment_turned_in_outlined,
    group: 'TOOLS',
  ),
  ShellDestination(
    key: 'pet',
    label: 'Pet Feeder',
    icon: Icons.pets_outlined,
    group: 'TOOLS',
  ),
  ShellDestination(
    key: 'attributes',
    label: 'Attributes',
    icon: Icons.list_alt_outlined,
    group: 'SYSTEM',
  ),
  ShellDestination(
    key: 'arduino',
    label: 'Arduino',
    icon: Icons.memory_outlined,
    group: 'SYSTEM',
  ),
  ShellDestination(
    key: 'hotkeys',
    label: 'Hotkeys',
    icon: Icons.keyboard_outlined,
    group: 'SYSTEM',
  ),
  ShellDestination(
    key: 'setup',
    label: 'Setup',
    icon: Icons.settings_outlined,
    group: 'SYSTEM',
  ),
  ShellDestination(
    key: 'calibrate',
    label: 'Calibrate',
    icon: Icons.straighten_outlined,
    group: 'SYSTEM',
  ),
];

class ShellRail extends StatelessWidget {
  const ShellRail({
    super.key,
    required this.selected,
    required this.onSelect,
    required this.runningCount,
  });

  final int selected;
  final ValueChanged<int> onSelect;
  final int runningCount;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final items = <Widget>[];
    String? group;
    for (var index = 0; index < shellDestinations.length; index++) {
      final destination = shellDestinations[index];
      if (destination.group != group) {
        group = destination.group;
        items.add(
          Padding(
            padding: const EdgeInsets.fromLTRB(
              ShellSpace.sm,
              ShellSpace.md,
              ShellSpace.sm,
              ShellSpace.xs,
            ),
            child: Text(
              group,
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
                letterSpacing: 0.8,
              ),
            ),
          ),
        );
      }
      items.add(
        _RailRow(
          destination: destination,
          selected: index == selected,
          badge: index == 0 ? runningCount : 0,
          onTap: () => onSelect(index),
        ),
      );
    }
    return Container(
      width: ShellWidth.rail,
      color: theme.colorScheme.surfaceContainerLow,
      child: ListView(
        key: const ValueKey('shell-rail'),
        padding: const EdgeInsets.symmetric(
          horizontal: ShellSpace.sm,
          vertical: ShellSpace.sm,
        ),
        children: items,
      ),
    );
  }
}

class _RailRow extends StatelessWidget {
  const _RailRow({
    required this.destination,
    required this.selected,
    required this.badge,
    required this.onTap,
  });

  final ShellDestination destination;
  final bool selected;
  final int badge;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.xxs),
      child: Material(
        color: selected ? colors.primaryContainer : Colors.transparent,
        borderRadius: BorderRadius.circular(ShellRadius.s),
        child: InkWell(
          key: ValueKey('rail-${destination.key}'),
          borderRadius: BorderRadius.circular(ShellRadius.s),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: ShellSpace.sm,
              vertical: ShellSpace.sm,
            ),
            child: Row(
              children: [
                Icon(
                  destination.icon,
                  size: ShellSize.iconMd,
                  color: selected ? colors.onPrimaryContainer : colors.onSurfaceVariant,
                ),
                const SizedBox(width: ShellSpace.sm),
                Expanded(
                  child: Text(
                    destination.label,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: selected
                          ? colors.onPrimaryContainer
                          : colors.onSurface,
                    ),
                  ),
                ),
                if (badge > 0)
                  Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: ShellSpace.xs + ShellSpace.xxs,
                      vertical: 1,
                    ),
                    decoration: BoxDecoration(
                      color: selected
                          ? colors.onPrimaryContainer
                          : context.shellColors.success,
                      borderRadius: BorderRadius.circular(ShellRadius.chip),
                    ),
                    child: Text(
                      '$badge',
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: selected
                            ? colors.primaryContainer
                            : context.shellColors.onSuccess,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
