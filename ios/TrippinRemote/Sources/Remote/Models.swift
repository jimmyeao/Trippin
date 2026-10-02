import Foundation

/// One entry of the server's advertised action list.
struct RemoteAction: Identifiable, Hashable {
    let id: JSON      // sent back verbatim in {"cmd":"action","action":…}
    let label: String
    var key: String { id.label }
}

/// The `hello` reply: what this Trippin can do.
struct ServerInfo {
    var version = ""
    var addr = ""
    var scenes: [String] = []
    var heavy: [Bool] = []
    var clips: [String] = []
    var palettes: [String] = []
    var actions: [RemoteAction] = []

    init() {}
    init(_ j: JSON) {
        version = j["version"].string ?? ""
        addr = j["addr"].string ?? ""
        scenes = j["scenes"].array.compactMap(\.string)
        heavy = j["heavy"].array.map { $0.bool ?? false }
        clips = j["clips"].array.compactMap(\.string)
        palettes = j["palettes"].array.compactMap(\.string)
        actions = j["actions"].array.map {
            RemoteAction(id: $0["id"], label: $0["label"].string ?? $0["id"].label)
        }
    }

    func action(_ key: String) -> RemoteAction? { actions.first { $0.key == key } }
}

/// The loaded timeline song, if any (`state.song`).
struct SongState: Equatable {
    var playing = false
    var pos = 0.0
    var len: Double?
    var name: String?

    init?(_ j: JSON) {
        guard case .object = j else { return nil }
        playing = j["playing"].bool ?? false
        pos = j["pos"].double ?? 0
        len = j["len"].double
        name = j["name"].string
    }
}

/// A `state` frame (~10 Hz). Missing fields keep sane defaults.
struct ShowState {
    var bpm = 0.0
    var conf = 0.0
    var beatInBar = 0
    var fps = 0.0
    var silent = false
    var scene = -1
    var sceneName = ""
    var nextScene: Int?
    var nextSceneName: String?
    var barInScene = 0
    var barsTotal = 0
    var clip: String?
    var blackout = false
    var strobe = false
    /// nil from a server that doesn't echo it.
    var cutOnDrops: Bool?
    var song: SongState?
    var fx = "Off"
    var calm = 0.0
    var nowPlaying: String?
    var recOn = false
    var mode = ""
    var dancer = false
    var palette = ""
    var phraseBars = 16
    var fxAmt = 0.5
    var fxAuto = false
    var dancerSize = 0.7
    var dancerTrails = false
    var latencyMs = 0.0
    var npSize = 1.0
    var brandOn = false
    var brandOpacity = 1.0
    var tickerOn = false
    var tickerSpeed = 1.0
    var tickerText = ""

    init() {}
    init(_ j: JSON) {
        bpm = j["bpm"].double ?? 0
        conf = j["conf"].double ?? 0
        beatInBar = j["beat_in_bar"].int ?? 0
        fps = j["fps"].double ?? 0
        silent = j["silent"].bool ?? false
        scene = j["scene"].int ?? -1
        sceneName = j["scene_name"].string ?? ""
        nextScene = j["next_scene"].int
        nextSceneName = j["next_scene_name"].string
        barInScene = j["bar_in_scene"].int ?? 0
        barsTotal = j["bars_total"].int ?? 0
        clip = j["clip"].string
        blackout = j["blackout"].bool ?? false
        strobe = j["strobe"].bool ?? false
        cutOnDrops = j["cut_on_drops"].bool
        song = SongState(j["song"])
        fx = j["fx"].string ?? "Off"
        calm = j["calm"].double ?? 0
        nowPlaying = ShowState.track(j["np"])
        recOn = j["rec_on"].bool ?? false
        mode = j["mode"].label
        dancer = j["dancer"].bool ?? false
        palette = j["palette"].string ?? ""
        phraseBars = j["phrase_bars"].int ?? 16
        fxAmt = j["fx_amt"].double ?? 0.5
        fxAuto = j["fx_auto"].bool ?? false
        dancerSize = j["dancer_size"].double ?? 0.7
        dancerTrails = j["dancer_trails"].bool ?? false
        latencyMs = j["latency_ms"].double ?? 0
        npSize = j["np_size"].double ?? 1
        brandOn = j["brand_on"].bool ?? false
        brandOpacity = j["brand_opacity"].double ?? 1
        tickerOn = j["ticker_on"].bool ?? false
        tickerSpeed = j["ticker_speed"].double ?? 1
        tickerText = j["ticker_text"].string ?? ""
    }

    /// Now playing arrives as a string or as an object with artist/title.
    private static func track(_ j: JSON) -> String? {
        if let s = j.string { return s.isEmpty ? nil : s }
        let artist = j["artist"].string ?? "", title = j["title"].string ?? ""
        switch (artist.isEmpty, title.isEmpty) {
        case (true, true): return nil
        case (false, false): return "\(artist) — \(title)"
        default: return artist + title
        }
    }
}

/// The whole-frame post effects Trippin knows (`config::Fx`).
let fxModes = ["Off", "MirrorX", "MirrorY", "Quad", "Kaleido6", "Kaleido8"]
