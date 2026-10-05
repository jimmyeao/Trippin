#!/bin/bash
# Compile PackPulse.dll against a BUILT engine's Managed/ folder (no Unity project needed).
#   build.sh <TrippinStage.app/Contents/Resources/Data/Managed or TrippinStage_Data/Managed> <out.dll>
M=$1; OUT=$2
E=/Applications/Unity/Hub/Editor/6000.3.25f1/Unity.app/Contents/Resources/Scripting
"$E/NetCoreRuntime/dotnet" "$E/DotNetSdkRoslyn/csc.dll" -nologo -target:library -nostdlib -noconfig -optimize \
  -r:"$M/mscorlib.dll" -r:"$M/netstandard.dll" -r:"$M/System.dll" -r:"$M/System.Core.dll" \
  -r:"$M/UnityEngine.dll" -r:"$M/UnityEngine.CoreModule.dll" -r:"$M/UnityEngine.AssetBundleModule.dll" \
  -r:"$M/Assembly-CSharp.dll" \
  -out:"$OUT" "$(dirname "$0")/PackPulse.cs"
