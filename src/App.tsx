import { useState } from "react";
import { usePlayer } from "./hooks/usePlayer";
import UrlInput from "./components/UrlInput";
import AudioDeviceSelector from "./components/AudioDeviceSelector";
import PreviewCanvas from "./components/PreviewCanvas";
import PlayerControls from "./components/PlayerControls";

export default function App() {
  const { status, play, stop, pause, setAudioDevice, setVolume } = usePlayer();
  const [url, setUrl] = useState("");

  const handlePlay = () => {
    if (url.trim()) {
      play(url.trim());
    }
  };

  const isActive = status.status === "playing" || status.status === "paused" || status.status === "loading";

  return (
    <div className="min-h-screen bg-[#1a1a1a] text-white flex flex-col overflow-auto">
      <div className="flex-1 flex flex-col gap-3 p-3">
        {/* URL 入力 */}
        <UrlInput
          url={url}
          onChange={setUrl}
          onSubmit={handlePlay}
          disabled={false}
          isLoading={status.status === "loading"}
        />

        {/* プレビュー */}
        <PreviewCanvas />

        {/* プレイヤーコントロール */}
        <PlayerControls
          isPlaying={status.status === "playing" || status.status === "paused"}
          onPause={pause}
          onStop={stop}
        />

        {/* ステータス表示 */}
        <StatusBadge status={status} />

        {/* Spout/Syphon 出力インジケーター */}
        <OutputIndicator
          spoutActive={status.spout_active}
          syphonActive={status.syphon_active}
        />

        {/* オーディオデバイス選択 */}
        <AudioDeviceSelector
          onSelect={setAudioDevice}
          onVolumeChange={setVolume}
        />
      </div>

      {/* リサイズインジケーター */}
      <div className="absolute bottom-0 right-0 w-4 h-4 opacity-20 pointer-events-none">
        <svg viewBox="0 0 16 16" className="text-gray-600">
          <path
            d="M16 16L16 12L12 16L16 16ZM16 8L8 16L12 16L16 12L16 8Z"
            fill="currentColor"
          />
        </svg>
      </div>
    </div>
  );
}

// ─── ステータスバッジ ──────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: { status: string; error?: string } }) {
  const colorMap: Record<string, string> = {
    idle: "bg-gray-700 text-gray-300",
    loading: "bg-yellow-800 text-yellow-200",
    playing: "bg-green-800 text-green-200",
    paused: "bg-blue-800 text-blue-200",
    error: "bg-red-900 text-red-200",
  };

  const labelMap: Record<string, string> = {
    idle: "待機中",
    loading: "読み込み中...",
    playing: "再生中",
    paused: "一時停止",
    error: "エラー",
  };

  return (
    <div className={`rounded px-3 py-2 text-sm ${colorMap[status.status] ?? colorMap.idle}`}>
      {labelMap[status.status] ?? status.status}
      {status.error && <span className="ml-2 text-xs opacity-75">({status.error})</span>}
    </div>
  );
}

// ─── 出力インジケーター ───────────────────────────────────────────────────────

function OutputIndicator({
  spoutActive,
  syphonActive,
}: {
  spoutActive: boolean;
  syphonActive: boolean;
}) {
  return (
    <div className="flex gap-3 text-xs">
      <div className={`flex items-center gap-1 ${spoutActive ? "text-green-400" : "text-gray-600"}`}>
        <span className={`w-2 h-2 rounded-full ${spoutActive ? "bg-green-400" : "bg-gray-600"}`} />
        Spout
      </div>
      <div className={`flex items-center gap-1 ${syphonActive ? "text-green-400" : "text-gray-600"}`}>
        <span className={`w-2 h-2 rounded-full ${syphonActive ? "bg-green-400" : "bg-gray-600"}`} />
        Syphon
      </div>
    </div>
  );
}
