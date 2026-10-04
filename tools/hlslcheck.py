# Syntax-checks the HLSLPROGRAM blocks of Unity .shader files with glslangValidator (apt install glslang-tools).
# Usage: python3 tools/hlslcheck.py unity/TrippinStage/Assets/Trippin/Shaders/Land.shader ...
# Syntax-check a Unity .shader's HLSLPROGRAM blocks with glslangValidator's HLSL front end.
import re,sys,subprocess,os,tempfile
D=os.path.dirname(os.path.abspath(sys.argv[1]))
STUB='''
#define CBUFFER_START(n)
#define CBUFFER_END
#define UNITY_INITIALIZE_OUTPUT(t,o)
float3 _WorldSpaceCameraPos; float4x4 unity_MatrixVP; float4x4 unity_ObjectToWorld; float4x4 UNITY_MATRIX_I_VP; float4x4 UNITY_MATRIX_VP; float4x4 UNITY_MATRIX_V; float4x4 unity_CameraProjection; float4 _ScreenParams; float4 _Time; float4 _ProjectionParams;
float3 TransformObjectToWorld(float3 p){return mul(unity_ObjectToWorld,float4(p,1)).xyz;}
float4 TransformWorldToHClip(float3 p){return mul(unity_MatrixVP,float4(p,1));}
float4 TransformObjectToHClip(float3 p){return mul(unity_MatrixVP,mul(unity_ObjectToWorld,float4(p,1)));}
float3 TransformWorldToObject(float3 p){return p;}
float3 TransformObjectToWorldDir(float3 p){return p;}
float3 TransformObjectToWorldNormal(float3 p){return p;}
'''
ok=True
for f in sys.argv[1:]:
    src=open(f).read()
    blocks=re.findall(r'HLSLPROGRAM(.*?)ENDHLSL',src,re.S)
    for bi,b in enumerate(blocks):
        def inc(m):
            n=m.group(1)
            fp=os.path.join(D,n)
            if os.path.exists(fp): return open(fp).read()
            return ''
        b=re.sub(r'#include\s+"([^"]+)"',inc,b)
        b=re.sub(r'#pragma[^\n]*','',b)
        for stage,ent in (('vert','vert'),('frag','frag')):
            if not re.search(r'\b'+ent+r'\s*\(',b): continue
            code=STUB+b
            t=tempfile.NamedTemporaryFile('w',suffix='.'+stage,delete=False); t.write(code); t.close()
            r=subprocess.run(['glslangValidator','-D','-e',ent,'-V','-S',stage,'-o','/dev/null',t.name],capture_output=True,text=True)
            out=(r.stdout+r.stderr).strip()
            errs=[l for l in out.splitlines() if 'ERROR' in l]
            if errs:
                ok=False; print(f,'block',bi,stage,'FAILED'); print('\n'.join(errs[:12]))
            os.unlink(t.name)
print('OK' if ok else 'ERRORS')
