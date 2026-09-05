using System;
using System.Collections.Generic;
using SealTools.Core.Config;

namespace SealTools.Launcher;

// Calibration helpers: window-size scaling (auto-anchor) + the calibrator's save payload.

public static class Calibration
{
    // Scale OCR capture geometry (absolute pixel coords) by independent x/y factors.
    public static void ScaleOcr(OcrGeometry ocr, double scaleX, double scaleY)
    {
        ocr.Region.Left = Round(ocr.Region.Left * scaleX);
        ocr.Region.Top = Round(ocr.Region.Top * scaleY);
        ocr.Region.Width = Round(ocr.Region.Width * scaleX);
        ocr.Region.Height = Round(ocr.Region.Height * scaleY);

        ocr.GradeArea.X1 = Round(ocr.GradeArea.X1 * scaleX);
        ocr.GradeArea.X2 = Round(ocr.GradeArea.X2 * scaleX);
        ocr.GradeArea.Y1 = Round(ocr.GradeArea.Y1 * scaleY);
        ocr.GradeArea.Y2 = Round(ocr.GradeArea.Y2 * scaleY);

        ScalePairY(ocr.GradeY, scaleY);
        ScalePairY(ocr.AttrY, scaleY);
        ScalePairY(ocr.RemainingY, scaleY);
        ocr.RowHeight = Math.Max(1, Round(ocr.RowHeight * scaleY));
    }

    // Scale absolute click positions by independent x/y factors.
    public static void ScalePositions(Dictionary<string, List<int>> positions, double scaleX, double scaleY)
    {
        foreach (var key in positions.Keys)
        {
            var p = positions[key];
            if (p.Count >= 2)
            {
                p[0] = Round(p[0] * scaleX);
                p[1] = Round(p[1] * scaleY);
            }
        }
    }

    private static void ScalePairY(List<int> pair, double scaleY)
    {
        if (pair.Count >= 2)
        {
            pair[0] = Round(pair[0] * scaleY);
            pair[1] = Round(pair[1] * scaleY);
        }
    }

    private static int Round(double v) => (int)Math.Round(v);
}
