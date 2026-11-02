/// Where the quick dock sits on screen. The three form factors from the design study are the
/// three families here; the rail and the bay also have a left/right side so the player can put the
/// launcher on whichever screen edge they actually use.
library;

enum DockPlacement {
  rightRail('right_rail', 'Right edge'),
  leftRail('left_rail', 'Left edge'),
  bottomRight('bottom_right', 'Bottom right'),
  bottomLeft('bottom_left', 'Bottom left'),
  topCentre('top_centre', 'Top centre');

  const DockPlacement(this.wire, this.label);

  final String wire;
  final String label;

  bool get isRail =>
      this == DockPlacement.rightRail || this == DockPlacement.leftRail;

  /// The dock's logical window size. The content is the same in every form factor; only the shape
  /// of the window changes (tall rail, wider bay, widest island).
  (int, int) get size => switch (this) {
    DockPlacement.rightRail || DockPlacement.leftRail => (300, 480),
    DockPlacement.bottomRight || DockPlacement.bottomLeft => (340, 460),
    DockPlacement.topCentre => (380, 500),
  };

  /// The collapsed resting form: a vertical edge tab for the rails, a status pill for the bays and a
  /// slim top island. Clicking it expands the launcher again.
  (int, int) get collapsedSize => switch (this) {
    DockPlacement.rightRail || DockPlacement.leftRail => (46, 150),
    DockPlacement.bottomRight || DockPlacement.bottomLeft => (220, 44),
    DockPlacement.topCentre => (200, 36),
  };
}
