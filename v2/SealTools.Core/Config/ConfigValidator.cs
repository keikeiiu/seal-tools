using System;
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel.DataAnnotations;

namespace SealTools.Core.Config;

// DataAnnotations-based validation (per plan §1). Missing/invalid machine-specific
// values surface as a descriptive ConfigException before OCR or COM ports initialize.

internal static class ConfigValidator
{
    public static void Validate(AppConfig c)
    {
        var errors = new List<string>();
        ValidateObject(c, "", errors);

        // Relational checks DataAnnotations can't express declaratively.
        var ocr = c.Tuner.Ocr;
        var ga = ocr.GradeArea;
        if (ga.X2 <= ga.X1 || ga.Y2 <= ga.Y1)
            errors.Add("tuner.ocr.grade_area: x2/y2 must be greater than x1/y1");
        else if (ga.X1 < 0 || ga.Y1 < 0 || ga.X2 > ocr.Region.Width || ga.Y2 > ocr.Region.Height)
            errors.Add("tuner.ocr.grade_area must lie inside tuner.ocr.region " +
                $"(region is {ocr.Region.Width}x{ocr.Region.Height})");

        // OcrEngine indexes these as [0]/[1]; a short list is an IndexOutOfRange at scan time.
        RequireBand(errors, "tuner.ocr.grade_y", ocr.GradeY);
        RequireBand(errors, "tuner.ocr.attr_y", ocr.AttrY);
        RequireBand(errors, "tuner.ocr.remaining_y", ocr.RemainingY);
        RequireOptionalBand(errors, "tuner.ocr.attr_x", ocr.AttrX);
        RequireOptionalBand(errors, "tuner.ocr.remaining_x", ocr.RemainingX);

        if (errors.Count > 0)
            throw new ConfigException(
                "Config validation failed:\n  - " + string.Join("\n  - ", errors) +
                "\nCheck defaults.yaml and local.yaml.");
    }

    private static void ValidateObject(object obj, string path, List<string> errors)
    {
        var results = new List<ValidationResult>();
        Validator.TryValidateObject(obj, new ValidationContext(obj), results, validateAllProperties: true);
        foreach (var r in results)
            errors.Add(string.IsNullOrEmpty(path) ? (r.ErrorMessage ?? "invalid") : $"{path}: {r.ErrorMessage}");

        foreach (var prop in obj.GetType().GetProperties())
        {
            if (!prop.CanRead) continue;
            var type = prop.PropertyType;
            if (type.IsValueType || type == typeof(string)) continue;
            var value = prop.GetValue(obj);
            if (value == null) continue;

            if (value is IEnumerable enumerable)
            {
                foreach (var item in enumerable)
                {
                    if (item is null || item is string) continue;
                    if (item.GetType().IsClass)
                        ValidateObject(item, Append(path, prop.Name), errors);
                }
            }
            else if (value.GetType().IsClass)
            {
                ValidateObject(value, Append(path, prop.Name), errors);
            }
        }
    }

    // A required band is [top, bottom] (or [left, right]) and must be ordered.
    private static void RequireBand(List<string> errors, string name, List<int> band)
    {
        if (band.Count < 2)
            errors.Add($"{name}: needs 2 values; got {band.Count}");
        else if (band[1] <= band[0])
            errors.Add($"{name}: the second value must be greater than the first");
    }

    // An optional X band may be omitted entirely, but if present it must be a complete pair.
    private static void RequireOptionalBand(List<string> errors, string name, List<int> band)
    {
        if (band.Count is 1)
            errors.Add($"{name}: leave it out or give 2 values; got 1");
    }

    private static string Append(string path, string name)
        => string.IsNullOrEmpty(path) ? name : $"{path}.{name}";
}
