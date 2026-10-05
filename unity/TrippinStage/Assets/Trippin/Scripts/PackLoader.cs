// SPIKE (spike/pack-loader-m2, not for merge): load shows at runtime from
// `-packDir <dir>`. Each subfolder holds manifest.json:
//   {"show":"unity_pack_pulse","dll":"PackPulse.dll","bundle":"packpulse.bundle",
//    "files":[{"name":"PackPulse.dll","sha256":"..."},{"name":"packpulse.bundle","sha256":"..."}]}
// Every listed file is SHA-256 checked before anything is loaded (the real thing:
// an Ed25519-signed manifest). The DLL is Assembly.Load'ed from bytes, the bundle
// opened with AssetBundle.LoadFromFile, the KitShow subclass created on an INACTIVE
// GameObject (KitShow.Awake runs Build), wired like StageBuilder wires kit shows
// (fields copied from an existing KitShow), and appended to ShowManager.shows/names.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using UnityEngine;

namespace TrippinStage
{
    public static class PackLoader
    {
        [Serializable] class FileEntry { public string name; public string sha256; }
        [Serializable] class Manifest { public string show; public string dll; public string bundle; public FileEntry[] files; }

        public static void Load(ShowManager mgr)
        {
            string dir = null;
            var a = Environment.GetCommandLineArgs();
            for (int i = 0; i + 1 < a.Length; i++) if (a[i] == "-packDir") dir = a[i + 1];
            if (dir == null || !Directory.Exists(dir)) return;
            KitShow template = mgr.shows.Select(g => g != null ? g.GetComponent<KitShow>() : null).FirstOrDefault(k => k != null);
            if (template == null) { Debug.LogError("[PackLoader] no KitShow to copy wiring from"); return; }
            var shows = new List<GameObject>(mgr.shows);
            var names = new List<string>(mgr.names);
            foreach (var pd in Directory.GetDirectories(dir))
            {
                var sw = System.Diagnostics.Stopwatch.StartNew();
                string mpath = Path.Combine(pd, "manifest.json");
                if (!File.Exists(mpath)) continue;
                try
                {
                    var m = JsonUtility.FromJson<Manifest>(File.ReadAllText(mpath));
                    // Tamper check: every file must match its manifest hash.
                    foreach (var f in m.files)
                    {
                        string got = Sha256(Path.Combine(pd, f.name));
                        if (!string.Equals(got, f.sha256, StringComparison.OrdinalIgnoreCase))
                            throw new Exception($"hash mismatch on {f.name}: manifest {f.sha256}, file {got}");
                    }
                    var asm = Assembly.Load(File.ReadAllBytes(Path.Combine(pd, m.dll)));
                    var type = asm.GetTypes().FirstOrDefault(t => typeof(KitShow).IsAssignableFrom(t) && !t.IsAbstract)
                               ?? throw new Exception("no KitShow subclass in " + m.dll);
                    var bundle = AssetBundle.LoadFromFile(Path.Combine(pd, m.bundle))
                                 ?? throw new Exception("AssetBundle.LoadFromFile failed: " + m.bundle);
                    var go = new GameObject(m.show);
                    go.SetActive(false);
                    go.transform.SetParent(template.transform.parent, false);
                    var ks = (KitShow)go.AddComponent(type);
                    foreach (var fi in typeof(KitShow).GetFields(BindingFlags.Public | BindingFlags.Instance))
                        if (fi.Name != "pack") fi.SetValue(ks, fi.GetValue(template));
                    ks.pack = bundle;
                    shows.Add(go);
                    names.Add(m.show);
                    Debug.Log($"[PackLoader] loaded {m.show} ({type.FullName} from {m.dll}, bundle {m.bundle}: {string.Join(",", bundle.GetAllAssetNames())}) in {sw.ElapsedMilliseconds} ms");
                }
                catch (Exception e)
                {
                    Debug.LogError($"[PackLoader] refused {pd}: {e.Message}");
                }
            }
            mgr.shows = shows.ToArray();
            mgr.names = names.ToArray();
        }

        static string Sha256(string path)
        {
            using var h = SHA256.Create();
            using var fs = File.OpenRead(path);
            return BitConverter.ToString(h.ComputeHash(fs)).Replace("-", "").ToLowerInvariant();
        }
    }
}
