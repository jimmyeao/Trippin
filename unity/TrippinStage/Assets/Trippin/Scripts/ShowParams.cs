// Per-show controls that Trippin can set (Perform tab > show controls, and
// src/showparams.rs). A show exposes a knob by tagging a public float field:
//
//     [TrippinParam("Swirl", 0f, 60f)] public float swirl = 25f;
//
// At startup ShowManager lists every show's tagged fields (at most 8, in
// declaration order, the field's initial value as the default) into the
// manifest Trippin asks for with `-trippinParams <path>`. Each frame the
// feed's `knobs` array (same order, values for the show on air) is written
// back into that show's fields. With no feed, or an older Trippin that
// sends no knobs, the fields keep their own values.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Reflection;
using System.Text;
using UnityEngine;

namespace TrippinStage
{
    [AttributeUsage(AttributeTargets.Field)]
    public sealed class TrippinParamAttribute : Attribute
    {
        public readonly string label;
        public readonly float min, max;

        public TrippinParamAttribute(string label, float min, float max)
        {
            this.label = label;
            this.min = min;
            this.max = max;
        }
    }

    public static class ShowParams
    {
        struct Knob
        {
            public Component owner;
            public FieldInfo field;
            public TrippinParamAttribute attr;
            public float def;
        }

        static List<Knob>[] _knobs;

        /// Collect the tagged fields of every show and write the manifest.
        public static void Init(GameObject[] shows, string[] names)
        {
            _knobs = new List<Knob>[shows.Length];
            for (int i = 0; i < shows.Length; i++)
            {
                var list = new List<Knob>();
                foreach (var c in shows[i].GetComponentsInChildren<MonoBehaviour>(true))
                    foreach (var f in c.GetType().GetFields(BindingFlags.Public | BindingFlags.Instance))
                    {
                        var a = f.GetCustomAttribute<TrippinParamAttribute>();
                        if (a == null || f.FieldType != typeof(float) || list.Count >= 8) continue;
                        if (f.Name == "zoom" && c is KitShow ks && !ks.UsesCamera) continue;
                        list.Add(new Knob { owner = c, field = f, attr = a, def = (float)f.GetValue(c) });
                    }
                _knobs[i] = list;
            }
            var args = Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < args.Length; i++)
                if (args[i] == "-trippinParams") Write(args[i + 1], names);
        }

        /// The manifest: {"shows":[{"scene":..,"params":[{"id","label","min","max","def"}]}]}.
        static void Write(string path, string[] names)
        {
            var ci = CultureInfo.InvariantCulture;
            var sb = new StringBuilder("{\"shows\":[");
            for (int i = 0; i < _knobs.Length; i++)
            {
                if (i > 0) sb.Append(',');
                sb.Append("{\"scene\":\"").Append(names[i]).Append("\",\"params\":[");
                for (int k = 0; k < _knobs[i].Count; k++)
                {
                    var n = _knobs[i][k];
                    if (k > 0) sb.Append(',');
                    sb.Append("{\"id\":\"").Append(n.field.Name.ToLowerInvariant())
                      .Append("\",\"label\":\"").Append(n.attr.label.Replace("\"", "'"))
                      .Append("\",\"min\":").Append(n.attr.min.ToString("R", ci))
                      .Append(",\"max\":").Append(n.attr.max.ToString("R", ci))
                      .Append(",\"def\":").Append(n.def.ToString("R", ci)).Append('}');
                }
                sb.Append("]}");
            }
            sb.Append("]}");
            try
            {
                // Write beside and swap in, so Trippin never reads half a file.
                System.IO.File.WriteAllText(path + ".tmp", sb.ToString());
                if (System.IO.File.Exists(path)) System.IO.File.Delete(path);
                System.IO.File.Move(path + ".tmp", path);
            }
            catch (Exception e)
            {
                Debug.LogWarning($"[ShowParams] can't write {path}: {e.Message}");
            }
        }

        /// Push the feed's knob values into show `idx`'s fields.
        public static void Apply(int idx, float[] knobs)
        {
            if (_knobs == null || idx < 0 || idx >= _knobs.Length || knobs == null) return;
            var list = _knobs[idx];
            for (int k = 0; k < list.Count && k < knobs.Length; k++)
            {
                float v = knobs[k];
                if (float.IsNaN(v) || float.IsInfinity(v)) continue;
                list[k].field.SetValue(list[k].owner, Mathf.Clamp(v, list[k].attr.min, list[k].attr.max));
            }
        }
    }
}
