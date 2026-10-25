/// Small form widgets shared by the wave-1 tabs. They render and report edits; they never decide a
/// rule (the core owns every rule — PROTOCOL §4/§7, PLAN-V3-SHELL §5).
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';

/// A titled group of fields.
class EditorSection extends StatelessWidget {
  const EditorSection({super.key, required this.title, required this.children});

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.lg),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: theme.textTheme.titleSmall),
          const SizedBox(height: ShellSpace.sm),
          ...children,
        ],
      ),
    );
  }
}

/// A draft-bound field's slot: a STABLE key for tests and an inner key that changes with the draft's
/// seed revision.
///
/// The inner key is the deterministic answer to stale controllers: when the draft is re-seeded
/// (Cancel, reconcile, a save's echo), the field's state — and its `TextEditingController` — is
/// recreated from the new value instead of trusting `didUpdateWidget` to win a race with a focused
/// field. Keystrokes do not bump the revision, so typing never loses its state.
class DraftFieldSlot extends StatelessWidget {
  const DraftFieldSlot({
    super.key,
    required this.slotKey,
    required this.syncToken,
    required this.child,
  });

  final String slotKey;
  final int syncToken;
  final Widget child;

  @override
  Widget build(BuildContext context) => KeyedSubtree(
    key: ValueKey(slotKey),
    child: KeyedSubtree(
      key: ValueKey('$slotKey-seed-$syncToken'),
      child: child,
    ),
  );
}

/// A labelled text field bound to a draft value.
///
/// The controller is kept across rebuilds; it only re-syncs from [value] when the draft changed
/// underneath the player (Cancel/reconcile), not on the player's own keystrokes.
class DraftTextField extends StatefulWidget {
  const DraftTextField({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.helper,
    this.enabled = true,
    this.width,
  });

  final String label;
  final String value;
  final ValueChanged<String> onChanged;
  final String? helper;
  final bool enabled;
  final double? width;

  @override
  State<DraftTextField> createState() => _DraftTextFieldState();
}

class _DraftTextFieldState extends State<DraftTextField> {
  late final TextEditingController _controller = TextEditingController(
    text: widget.value,
  );
  final _focus = FocusNode();

  @override
  void didUpdateWidget(DraftTextField oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Resync when the DRAFT changed underneath the field (Cancel/reconcile/external config
    // change). A player's own keystroke already leaves controller.text == widget.value, so this
    // does not fight typing.
    if (widget.value != oldWidget.value && widget.value != _controller.text) {
      _setText(widget.value);
    }
  }

  void _setText(String text) {
    _controller.value = TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
  }

  @override
  void dispose() {
    _focus.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final field = TextField(
      controller: _controller,
      focusNode: _focus,
      enabled: widget.enabled,
      decoration: InputDecoration(
        labelText: widget.label,
        helperText: widget.helper,
        isDense: true,
        border: const OutlineInputBorder(),
      ),
      onChanged: widget.onChanged,
    );
    final width = widget.width;
    return width == null ? field : SizedBox(width: width, child: field);
  }
}

/// A labelled number field. Invalid text is ignored (the draft keeps its last valid number), so a
/// half-typed value never sends `null` to the core.
class DraftNumberField extends StatefulWidget {
  const DraftNumberField({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.integer = false,
    this.helper,
    this.width,
  });

  final String label;
  final num? value;
  final ValueChanged<num> onChanged;
  final bool integer;
  final String? helper;
  final double? width;

  @override
  State<DraftNumberField> createState() => _DraftNumberFieldState();
}

class _DraftNumberFieldState extends State<DraftNumberField> {
  late final TextEditingController _controller = TextEditingController(
    text: _format(widget.value),
  );
  final _focus = FocusNode();

  String _format(num? value) {
    if (value == null) {
      return '';
    }
    if (widget.integer || value == value.roundToDouble()) {
      return value.round().toString();
    }
    return value.toString();
  }

  @override
  void didUpdateWidget(DraftNumberField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.value == oldWidget.value) {
      return;
    }
    final formatted = _format(widget.value);
    if (_controller.text == formatted) {
      return;
    }
    // A player's own keystroke parses to the new draft value, so leave the text alone (it may be
    // "1.50" while the draft stores 1.5). Anything else — Cancel, reconcile, an external
    // config.changed — must be pushed into the field.
    final trimmed = _controller.text.trim();
    final parsed = widget.integer
        ? int.tryParse(trimmed)
        : double.tryParse(trimmed);
    if (parsed != null && parsed == widget.value) {
      return;
    }
    _setText(formatted);
  }

  void _setText(String text) {
    _controller.value = TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
  }

  @override
  void dispose() {
    _focus.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final field = TextField(
      controller: _controller,
      focusNode: _focus,
      keyboardType: TextInputType.number,
      decoration: InputDecoration(
        labelText: widget.label,
        helperText: widget.helper,
        isDense: true,
        border: const OutlineInputBorder(),
      ),
      onChanged: (text) {
        final trimmed = text.trim();
        if (trimmed.isEmpty) {
          return;
        }
        final parsed = widget.integer
            ? int.tryParse(trimmed)
            : double.tryParse(trimmed);
        if (parsed != null) {
          widget.onChanged(parsed);
        }
      },
    );
    final width = widget.width;
    return width == null ? field : SizedBox(width: width, child: field);
  }
}

/// A labelled switch bound to a draft boolean.
class DraftSwitchField extends StatelessWidget {
  const DraftSwitchField({
    super.key,
    required this.label,
    required this.value,
    required this.onChanged,
    this.helper,
  });

  final String label;
  final bool value;
  final ValueChanged<bool> onChanged;
  final String? helper;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SwitchListTile(
      contentPadding: EdgeInsets.zero,
      dense: true,
      title: Text(label, style: theme.textTheme.bodyMedium),
      subtitle: helper == null ? null : Text(helper!),
      value: value,
      onChanged: onChanged,
    );
  }
}

/// A labelled dropdown bound to a draft string.
class DraftDropdownField extends StatelessWidget {
  const DraftDropdownField({
    super.key,
    required this.label,
    required this.value,
    required this.options,
    required this.onChanged,
    this.width,
  });

  final String label;
  final String? value;
  final List<String> options;
  final ValueChanged<String?> onChanged;
  final double? width;

  @override
  Widget build(BuildContext context) {
    final current = options.contains(value) ? value : null;
    final dropdown = DropdownButtonFormField<String>(
      initialValue: current,
      isExpanded: true,
      isDense: true,
      decoration: InputDecoration(
        labelText: label,
        isDense: true,
        border: const OutlineInputBorder(),
      ),
      items: [
        for (final option in options)
          DropdownMenuItem(value: option, child: Text(option)),
      ],
      onChanged: onChanged,
    );
    final boxWidth = width;
    return boxWidth == null
        ? dropdown
        : SizedBox(width: boxWidth, child: dropdown);
  }
}

/// A labelled value the tab does not edit this wave (calibrated geometry is wave 2).
class ReadOnlyValue extends StatelessWidget {
  const ReadOnlyValue({super.key, required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.xs),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 220,
            child: Text(label, style: theme.textTheme.bodySmall),
          ),
          Expanded(
            child: Text(
              value,
              style: theme.textTheme.bodySmall?.copyWith(
                fontFamily: 'Consolas',
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// A value as the read-only rows show it (`[x, y, w, h]` style, or a dash).
String readOnlyText(Object? value) {
  if (value == null) {
    return '—';
  }
  return '${cloneJson(value)}';
}
