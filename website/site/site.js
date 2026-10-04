// Point download buttons at the latest release assets (falls back to the releases page).
fetch("https://api.github.com/repos/jimmyeao/Trippin/releases/latest").then(r=>r.ok?r.json():null).then(j=>{
  if(!j) return;
  const f=re=>(j.assets||[]).find(a=>re.test(a.name));
  const set=(sel,a)=>{ if(a) document.querySelectorAll(sel).forEach(e=>e.href=a.browser_download_url) };
  set(".dl-win",f(/^Trippin-Setup-.*\.exe$/i)); set(".dl-mac",f(/^Trippin-macOS-.*\.pkg$/i)); set(".dl-mac-zip",f(/^Trippin-macOS-.*\.zip$/i));
  set(".dl-agent-win",f(/^TrippinAgent-Windows-.*\.zip$/i)); set(".dl-agent-mac",f(/^TrippinAgent-macOS-.*\.zip$/i));
  const v=j.tag_name.replace(/^v/,""), d=new Date(j.published_at).toLocaleDateString(undefined,{year:"numeric",month:"short",day:"numeric"});
  document.querySelectorAll(".ver").forEach(e=>e.textContent="Version "+v+" · "+d);
}).catch(()=>{});
document.getElementById("yr").textContent=new Date().getFullYear();
const io=new IntersectionObserver(es=>es.forEach(e=>{if(e.isIntersecting){e.target.classList.add("in");io.unobserve(e.target)}}),{threshold:.12});
document.querySelectorAll(".reveal").forEach(el=>io.observe(el));
// videos: play only when visible
const vio=new IntersectionObserver(es=>es.forEach(e=>{const v=e.target;if(e.isIntersecting){v.preload="auto";v.play().catch(()=>{})}else v.pause()}),{threshold:.25});
document.querySelectorAll("video").forEach(v=>vio.observe(v));
// Hero reel: name the scene on screen, synced to the cut list.
(function(){
  const v=document.querySelector(".reel-bg video"), tag=document.getElementById("reel-scene");
  if(!v||!tag) return;
  fetch("img/reel.json").then(r=>r.json()).then(j=>{
    let i=-1;
    const tick=()=>{
      const t=v.currentTime; let k=0;
      for(let n=0;n<j.cuts.length;n++){ if(j.cuts[n][0]<=t) k=n; }
      if(k!==i){ i=k; tag.textContent=j.cuts[k][1]; }
      requestAnimationFrame(tick);
    };
    tick();
  }).catch(()=>{});
})();
// Click a screenshot to enlarge it; click it again for 1:1 pixels (scroll to pan). Esc or a click outside closes.
(function(){
  const sel=".frame img,.tablet img,.phones .phone img,.gal img";
  let ov=null;
  const close=()=>{ if(ov){ ov.remove(); ov=null; document.documentElement.style.overflow=""; } };
  const open=im=>{
    ov=document.createElement("div"); ov.className="lightbox";
    const big=document.createElement("img"); big.src=im.currentSrc||im.src; big.alt=im.alt||"";
    const hint=document.createElement("div"); hint.className="hint"; hint.textContent="Click the image for full size · Esc to close";
    ov.append(big,hint);
    ov.addEventListener("click",e=>{ if(e.target===big){ ov.classList.toggle("full"); } else close(); });
    document.body.append(ov); document.documentElement.style.overflow="hidden";
  };
  document.addEventListener("keydown",e=>{ if(e.key==="Escape") close(); });
  document.addEventListener("click",e=>{ const im=e.target.closest&&e.target.closest(sel); if(im&&im.tagName==="IMG"&&!ov) open(im); });
})();
