import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { usePlayer } from "./hooks/usePlayer";
import UrlInput from "./components/UrlInput";
import TransportControls from "./components/TransportControls";
import AudioDeviceSelector from "./components/AudioDeviceSelector";
import PreviewCanvas from "./components/PreviewCanvas";

export default function App() {
  const { status, play, stop, pause, setAudioDevice, setVolume } = usePlayer();
  const [url, setUrl] = useState("");

  const handlePlay = () => {
    if (url.trim()) {
      play(url.trim());
    }
  };

  const isActive = status.status === "playing" || status.status === "paused" || status.status === "loading";

  const appWindow = getCurrentWindow();

  return (
    <div className="min-h-screen bg-[#1a1a1a] text-white flex flex-col">
      {/* タイトルバー (ドラッグ可能) */}
      <div
        data-tauri-drag-region
        className="h-8 bg-[#111] flex items-center justify-between px-3 text-xs text-gray-500 select-none"
      >
        <span className="text-gray-400">yt-spout-syphon-bridge</span>
        <div className="flex items-center gap-2">
          <button
            onClick={() => appWindow.minimize()}
            className="w-5 h-5 rounded hover:bg-gray-700 flex items-center justify-center transition-colors"
            title="最小化"
          >
            <span className="text-gray-400">−</span>
          </button>
          <button
            onClick={() => appWindow.close()}
            className="w-5 h-5 rounded hover:bg-red-600 flex items-center justify-center transition-colors"
            title="閉じる"
          >
            <span className="text-gray-400 hover:text-white">×</span>
          </button>
        </div>
      </div>

      <div className="flex-1 flex flex-col gap-4 p-4">
        {/* URL 入力 */}
        <UrlInput
          url={url}
          onChange={setUrl}
          onSubmit={handlePlay}
          disabled={status.status === "loading"}
        />

        {/* 再生コントロール */}
        <TransportControls
          status={status}
          onPlay={handlePlay}
          onStop={stop}
          onPause={pause}
          disabled={!url.trim() && !isActive}
        />

        {/* プレビュー */}
        <PreviewCanvas />

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
