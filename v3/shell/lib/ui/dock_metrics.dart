import 'package:flutter/material.dart';

import '../app/dock_placement.dart';

/// The quick dock's size tokens. [DockSize] scales the window; this table scales the UI inside it.
///
/// Keeping the two factors separate is deliberate: a 1.25× window can afford 15 px titles and a
/// 76 × 34 Start button, while a 0.9× window must keep its labels readable rather than shrinking
/// them 0.9×.
@immutable
class DockMetrics {
  const DockMetrics({
    required this.tileHeight,
    required this.tilePadX,
    required this.tilePadY,
    required this.tileGap,
    required this.iconTile,
    required this.iconSize,
    required this.titleSize,
    required this.summarySize,
    required this.actionWidth,
    required this.actionHeight,
    required this.actionLabelSize,
    required this.optionsSize,
    required this.headerControl,
    required this.headerIconSize,
    required this.headerLabelSize,
    required this.headerPadTop,
    required this.headerPadBottom,
    required this.footerButtonHeight,
    required this.footerLabelSize,
    required this.footerIconSize,
    required this.footerPadTop,
    required this.footerPadBottom,
    required this.sectionLabelSize,
    required this.longTitleSize,
    required this.longStateSize,
    required this.longIconSize,
    required this.collapsedToolBox,
    required this.collapsedIconSize,
    required this.collapsedControl,
    required this.collapsedExpandIcon,
    required this.dragIconSize,
    required this.dotSize,
    required this.editControl,
    required this.menuTitleSize,
    required this.menuCaptionSize,
    required this.radius,
  });

  final double tileHeight;
  final double tilePadX;
  final double tilePadY;
  final double tileGap;
  final double iconTile;
  final double iconSize;
  final double titleSize;
  final double summarySize;
  final double actionWidth;
  final double actionHeight;
  final double actionLabelSize;
  final double optionsSize;
  final double headerControl;
  final double headerIconSize;
  final double headerLabelSize;
  final double headerPadTop;
  final double headerPadBottom;
  final double footerButtonHeight;
  final double footerLabelSize;
  final double footerIconSize;
  final double footerPadTop;
  final double footerPadBottom;
  final double sectionLabelSize;
  final double longTitleSize;
  final double longStateSize;
  final double longIconSize;
  final double collapsedToolBox;
  final double collapsedIconSize;
  final double collapsedControl;
  final double collapsedExpandIcon;
  final double dragIconSize;
  final double dotSize;
  final double editControl;
  final double menuTitleSize;
  final double menuCaptionSize;
  final double radius;

  TextStyle titleStyle(ThemeData theme) =>
      theme.textTheme.titleSmall!.copyWith(fontSize: titleSize, height: 1.15);

  TextStyle summaryStyle(ThemeData theme) =>
      theme.textTheme.labelSmall!.copyWith(
        fontSize: summarySize,
        color: theme.colorScheme.onSurfaceVariant,
      );

  TextStyle actionStyle(ThemeData theme) =>
      theme.textTheme.labelSmall!.copyWith(
        fontSize: actionLabelSize,
        fontWeight: FontWeight.w600,
        height: 1,
      );

  TextStyle sectionStyle(ThemeData theme) =>
      theme.textTheme.labelSmall!.copyWith(
        fontSize: sectionLabelSize,
        letterSpacing: 0.06,
        color: theme.colorScheme.onSurfaceVariant,
      );

  TextStyle longTitleStyle(ThemeData theme) =>
      theme.textTheme.bodySmall!.copyWith(
        fontSize: longTitleSize,
        color: theme.colorScheme.onSurfaceVariant,
      );

  TextStyle longStateStyle(ThemeData theme) =>
      theme.textTheme.labelSmall!.copyWith(fontSize: longStateSize);

  TextStyle footerStyle(ThemeData theme) =>
      theme.textTheme.labelLarge!.copyWith(fontSize: footerLabelSize);

  static DockMetrics forSize(DockSize size) => switch (size) {
    DockSize.compact => const DockMetrics(
      tileHeight: 40,
      tilePadX: 6,
      tilePadY: 2,
      tileGap: 5,
      iconTile: 28,
      iconSize: 15,
      titleSize: 12.5,
      summarySize: 10,
      actionWidth: 58,
      actionHeight: 26,
      actionLabelSize: 11.5,
      optionsSize: 26,
      headerControl: 24,
      headerIconSize: 14,
      headerLabelSize: 9,
      headerPadTop: 5,
      headerPadBottom: 3,
      footerButtonHeight: 30,
      footerLabelSize: 12,
      footerIconSize: 14,
      footerPadTop: 4,
      footerPadBottom: 6,
      sectionLabelSize: 9,
      longTitleSize: 11.5,
      longStateSize: 9.5,
      longIconSize: 13,
      collapsedToolBox: 24,
      collapsedIconSize: 14,
      collapsedControl: 24,
      collapsedExpandIcon: 15,
      dragIconSize: 12,
      dotSize: 5,
      editControl: 24,
      menuTitleSize: 13,
      menuCaptionSize: 9.5,
      radius: 6,
    ),
    DockSize.standard => const DockMetrics(
      tileHeight: 44,
      tilePadX: 8,
      tilePadY: 4,
      tileGap: 6,
      iconTile: 30,
      iconSize: 16,
      titleSize: 13,
      summarySize: 10.5,
      actionWidth: 64,
      actionHeight: 28,
      actionLabelSize: 12,
      optionsSize: 28,
      headerControl: 26,
      headerIconSize: 15,
      headerLabelSize: 9.5,
      headerPadTop: 6,
      headerPadBottom: 4,
      footerButtonHeight: 32,
      footerLabelSize: 12,
      footerIconSize: 14,
      footerPadTop: 4,
      footerPadBottom: 8,
      sectionLabelSize: 9.5,
      longTitleSize: 12,
      longStateSize: 10,
      longIconSize: 14,
      collapsedToolBox: 26,
      collapsedIconSize: 15,
      collapsedControl: 26,
      collapsedExpandIcon: 16,
      dragIconSize: 14,
      dotSize: 6,
      editControl: 24,
      menuTitleSize: 13.5,
      menuCaptionSize: 10,
      radius: 7,
    ),
    DockSize.large => const DockMetrics(
      tileHeight: 54,
      tilePadX: 10,
      tilePadY: 6,
      tileGap: 8,
      iconTile: 36,
      iconSize: 19,
      titleSize: 15,
      summarySize: 11.5,
      actionWidth: 76,
      actionHeight: 34,
      actionLabelSize: 13.5,
      optionsSize: 32,
      headerControl: 30,
      headerIconSize: 19,
      headerLabelSize: 11,
      headerPadTop: 8,
      headerPadBottom: 6,
      footerButtonHeight: 40,
      footerLabelSize: 13.5,
      footerIconSize: 18,
      footerPadTop: 6,
      footerPadBottom: 10,
      sectionLabelSize: 11,
      longTitleSize: 13,
      longStateSize: 11,
      longIconSize: 18,
      collapsedToolBox: 32,
      collapsedIconSize: 18,
      collapsedControl: 32,
      collapsedExpandIcon: 19,
      dragIconSize: 17,
      dotSize: 7,
      editControl: 30,
      menuTitleSize: 15,
      menuCaptionSize: 11,
      radius: 8,
    ),
  };
}
