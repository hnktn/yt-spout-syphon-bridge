import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { usePlayer } from "./hooks/usePlayer";
import UrlInput from "./components/UrlInput";
import AudioDeviceSelector from "./components/AudioDeviceSelector";
import PreviewCanvas from "./components/PreviewCanvas";
import PlayerControls from "./components/PlayerControls";

// プレビュー表示・非表示それぞれのウィンドウ高さ
const HEIGHT_WITH_PREVIEW = 532;
const HEIGHT_WITHOUT_PREVIEW = 332;

export default function App() {
  const { status, play, stop, pause, setAudioDevice, setVolume } = usePlayer();
  const [url, setUrl] = useState("");
  const [previewVisible, setPreviewVisible] = useState(true);

  const handlePlay = () => {
    if (url.trim()) {
      play(url.trim());
    }
  };

  const handlePreviewToggle = async () => {
    const next = !previewVisible;
    setPreviewVisible(next);
    // プレビュー表示状態に応じてウィンドウ高さを変更
    const win = getCurrentWindow();
    await win.setSize(new LogicalSize(360, next ? HEIGHT_WITH_PREVIEW : HEIGHT_WITHOUT_PREVIEW));
  };

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
          isPlaying={status.status === "playing" || status.status === "paused"}
        />

        {/* プレビュー */}
        <PreviewCanvas visible={previewVisible} onToggle={handlePreviewToggle} />

        {/* プレイヤーコントロール */}
        <PlayerControls
          isPlaying={status.status === "playing" || status.status === "paused"}
          onPause={pause}
          onStop={stop}
        />

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
      <div className={`flex items-center gap-1.5 ${spoutActive ? "text-accent-green" : "text-text-muted"}`}>
        <span className={`w-1.5 h-1.5 rounded-full ${spoutActive ? "bg-accent-green" : "bg-text-muted"}`} />
        <span className="uppercase tracking-wide">SPOUT</span>
      </div>
      <div className="w-px bg-surface-border" />
      <div className={`flex items-center gap-1.5 ${syphonActive ? "text-accent-green" : "text-text-muted"}`}>
        <span className={`w-1.5 h-1.5 rounded-full ${syphonActive ? "bg-accent-green" : "bg-text-muted"}`} />
        <span className="uppercase tracking-wide">SYPHON</span>
      </div>
    </div>
  );
}
