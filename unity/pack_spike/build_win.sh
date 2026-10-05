#!/bin/bash
# Windows (Git Bash) version of build.sh: compile PackPulse.dll against a BUILT engine's Managed/ folder.
#   build_win.sh <TrippinStage_Data/Managed> <out.dll>
M=$(cygpath -w "$1"); OUT=$(cygpath -w "$2")
E="/c/Program Files/Unity/Hub/Editor/6000.3.25f1/Editor/Data"
"$E/NetCoreRuntime/dotnet.exe" "$(cygpath -w "$E/DotNetSdkRoslyn/csc.dll")" -nologo -target:library -nostdlib -noconfig -optimize \
  -r:"$M\\mscorlib.dll" -r:"$M\\netstandard.dll" -r:"$M\\System.dll" -r:"$M\\System.Core.dll" \
  -r:"$M\\UnityEngine.dll" -r:"$M\\UnityEngine.CoreModule.dll" -r:"$M\\UnityEngine.AssetBundleModule.dll" \
  -r:"$M\\Assembly-CSharp.dll" \
  -out:"$OUT" "$(cygpath -w "$(dirname "$0")/PackPulse.cs")"
