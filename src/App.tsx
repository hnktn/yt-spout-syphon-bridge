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
    <div className="min-h-screen bg-surface font-mono text-text-primary flex flex-col overflow-auto">
      <div className="flex-1 flex flex-col gap-2 p-2">
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
      <div className="absolute bottom-0 right-0 w-3 h-3 opacity-10 pointer-events-none">
        <svg viewBox="0 0 16 16" className="text-text-muted">
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
    idle: "bg-surface-1 text-text-secondary",
    loading: "bg-surface-2 text-text-primary",
    playing: "bg-surface-2 text-accent",
    paused: "bg-surface-1 text-text-secondary",
    error: "bg-surface-2 text-accent",
  };

  const labelMap: Record<string, string> = {
    idle: "IDLE",
    loading: "LOADING",
    playing: "PLAYING",
    paused: "PAUSED",
    error: "ERROR",
  };

  return (
    <div className={`border border-surface-border rounded-sm px-2 py-1 text-xs uppercase tracking-wide ${colorMap[status.status] ?? colorMap.idle}`}>
      {labelMap[status.status] ?? status.status}
      {status.error && <span className="ml-1 opacity-50 normal-case">({status.error})</span>}
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
    <div className="flex gap-2 text-xs bg-surface-1 border border-surface-border rounded-sm p-1.5">
      <div className={`flex items-center gap-1.5 ${spoutActive ? "text-accent" : "text-text-muted"}`}>
        <span className={`w-1 h-1 ${spoutActive ? "bg-accent" : "bg-text-muted"}`} />
        <span className="uppercase tracking-wide">SPOUT</span>
      </div>
      <div className="w-px bg-surface-border" />
      <div className={`flex items-center gap-1.5 ${syphonActive ? "text-accent" : "text-text-muted"}`}>
        <span className={`w-1 h-1 ${syphonActive ? "bg-accent" : "bg-text-muted"}`} />
        <span className="uppercase tracking-wide">SYPHON</span>
      </div>
    </div>
  );
}
