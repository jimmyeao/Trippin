<section id="timeline" class="dive">
  <div class="wrap">
    <div class="head reveal">
      <span class="tag">Timeline editor + AI</span>
      <h2>Script a whole set, or let AI direct it</h2>
      <p class="sec-lead">Load a track and Trippin analyses it offline: waveform, beat and bar grid anchored to the real downbeat, and the structure of the song. Then build the show by hand, record it live, or have an AI write the cue list for you.</p>
    </div>
    <div class="frame reveal" style="padding:0;background:#07060d">
      <video autoplay muted loop playsinline preload="none" poster="img/features/editor-reel-poster.jpg" style="width:100%;display:block"><source src="img/features/editor-reel.mp4" type="video/mp4"></video>
    </div>
    <p class="sub reveal" style="text-align:center;margin:12px 0 28px">The real timeline editor playing a 6-minute show: scene cuts on the phrase, dancer routines, palette and effect changes, all landing on the beat.</p>
    <div class="frame reveal"><img loading="lazy" src="img/features/timeline-editor.webp" alt="The whole show in the timeline editor: waveform, 130 scene thumbnails, and lanes for dancer, effects and show cues" width="1800" height="446"></div>
    <p class="sub reveal" style="text-align:center;margin:12px 0 0">The whole show at a glance. Every block is draggable, resizable and yours to change.</p>

    <div class="pipe reveal" style="margin-top:22px">
      <div><b>Analyse</b>Per-bar energy, onsets, vocals and "air", all computed on your machine.</div>
      <div><b>Segment</b>Intro, groove, build, drop, peak, breakdown, outro, found by where the <em>sound</em> changes.</div>
      <div><b>Direct</b>The model picks scenes by measured visual energy, with extra cuts through builds.</div>
      <div><b>Enforce</b>Trippin's own show rules fix the plan: calm in breakdowns, a change on every drop.</div>
      <div><b>Edit</b>Everything lands as cues you can drag, retime or delete. It's your show.</div>
    </div>

    <div class="grid g3" style="margin-top:34px">
      <div class="card reveal"><h3>Drum-fill aware</h3><p>Rolls and snare runs mark phrase ends. The AI places strobe, stutter and flash hits on the fill's real drum hits, never as a flash on tunnel scenes.</p></div>
      <div class="card reveal"><h3>Text as performance</h3><p>A word per beat on the drop, hook words landed on their beat, and punch, shake, bounce or shatter effects. If there's no text, it hits the track title on the first drop.</p></div>
      <div class="card reveal"><h3>Record mode</h3><p>Arm record, play the track, and perform with your hotkeys, MIDI pads or the remote. Every move lands on the strip as a cue. Save it, replay it, tweak it.</p></div>
      <div class="card reveal"><h3>Follow live</h3><p>Playing the same song in the room? Trippin locks onto the live audio and fires the pre-programmed show in step with the DJ's deck.</p></div>
      <div class="card reveal"><h3>Bring your own AI</h3><p>Anthropic, OpenAI, Gemini, or any OpenAI-compatible endpoint including local Ollama. Paste your key in Settings.</p></div>
      <div class="card reveal"><h3>Your audio stays yours</h3><p>No audio is ever uploaded. Only a numeric summary of the track and its title leave your machine, and only when you ask for an AI show.</p></div>
    </div>
  </div>
</section>

<section id="midi" class="dive" style="background:var(--bg2)">
  <div class="wrap split">
    <div class="reveal">
      <span class="tag">MIDI mapping</span>
      <h2>Every action on a pad</h2>
      <p class="sec-lead">Plug in any pad or key controller. Pick it, click <b>midi</b> next to an action, hit a pad, and it's bound. No config files, no drivers to learn.</p>
      <ul class="ticks">
        <li><span><b>Everything is mappable:</b> scene cuts, strobe, blackout, dancer, palettes, effects, save clip, record set and more.</span></li>
        <li><span><b>One-press learn.</b> Re-learning a note simply steals it, so one pad is always one action.</span></li>
        <li><span><b>Records into your show.</b> Pad hits land on the timeline like hotkeys when you record.</span></li>
        <li><span><b>Hot-plug friendly.</b> Unplug and replug mid-set and it reconnects on its own.</span></li>
        <li><span><b>Any MIDI channel</b>, with clean press-only triggering so toggles never double-fire.</span></li>
      </ul>
    </div>
    <div class="reveal"><div class="frame"><img loading="lazy" src="img/features/midi-keys.webp" alt="The Keys page: every action with a keyboard binding, a rebind button and a midi learn button" width="1084" height="660"></div>
      <p class="sub" style="text-align:center;margin-top:12px">The Keys page. Click <b>midi</b> next to any action, hit a pad, done.</p></div>
  </div>
</section>

<section id="output" class="dive">
  <div class="wrap">
    <div class="head center reveal">
      <span class="tag">NDI &amp; Spout</span>
      <h2>Into OBS, Resolume or vMix in a click</h2>
      <p class="sec-lead">The finished frame (scene, dancer, effects, overlays, text) goes out as a live video source. Choose the route that fits your setup.</p>
    </div>
    <div class="diagram reveal">
      <div class="route">
        <div class="node hot">Trippin<br><small style="color:#fff;opacity:.85;font-weight:500">scene + dancer + overlays</small></div>
        <div class="stack">
          <div class="lane"><b>Spout</b><small>Same PC · GPU texture sharing · zero network, near-zero latency</small></div>
          <div class="lane"><b>NDI</b><small>Over your network · to another PC or a streaming machine</small></div>
        </div>
        <div class="stack">
          <div class="node"><b>OBS</b><small>Spout2 Capture / NDI Source</small></div>
          <div class="node"><b>Resolume · vMix · TouchDesigner</b><small>anything that reads Spout or NDI</small></div>
        </div>
      </div>
    </div>
    <div class="frame narrow2 reveal" style="max-width:640px;margin:22px auto 0"><img loading="lazy" src="img/features/video-output.webp" alt="The Video output card: Spout and NDI toggles, scenes or transparent background, 720p to 2160p, 30 or 60 fps" width="1056" height="456"></div>
    <p class="sub reveal" style="text-align:center;margin:10px 0 0">The Video output card on the Stream tab.</p>
    <div class="grid g3" style="margin-top:22px">
      <div class="card reveal"><h3>720p to 4K, 30 or 60 fps</h3><p>Output size and frame rate are independent of the window. If a receiver falls behind, frames drop. Your visuals never stutter.</p></div>
      <div class="card reveal"><h3>Transparent mode</h3><p>Turn the scenes off and send only the dancer, glow, overlays and text with true alpha. Layer Trippin straight over your camera in OBS.</p></div>
      <div class="card reveal"><h3>Native Spout, no plugins on our side</h3><p>Spout is built in, with no extra DLLs. NDI loads the free runtime on demand, and the app works fine without it.</p></div>
    </div>
  </div>
</section>

