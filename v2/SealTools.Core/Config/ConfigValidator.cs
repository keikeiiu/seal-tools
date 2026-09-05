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

        // Relational check DataAnnotations can't express declaratively.
        var ga = c.Tuner.Ocr.GradeArea;
        if (ga.X2 <= ga.X1 || ga.Y2 <= ga.Y1)
            errors.Add("tuner.ocr.grade_area: x2/y2 must be greater than x1/y1");

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

    private static string Append(string path, string name)
        => string.IsNullOrEmpty(path) ? name : $"{path}.{name}";
}
